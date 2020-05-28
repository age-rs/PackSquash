mod micro_zip;
mod resource_pack_file;

use std::error::Error;
use std::io::Write;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{cmp, env, fs, process};

use threadpool::ThreadPool;

use clap::{App, ArgMatches};
use pbr::MultiBar;
use simple_error::SimpleError;

use micro_zip::MicroZip;
use micro_zip::ZipFileType;

fn main() {
	let parameters = App::new("PackSquash")
		.version(env!("CARGO_PKG_VERSION"))
		.author(env!("CARGO_PKG_AUTHORS"))
		.about("Lossily compresses and prepares Minecraft resource packs for distribution")
		.args_from_usage("
			[skip pack icon] -n --skip-pack-icon 'If specified, the pack icon in pack.png will be skipped and not included in the resulting file'
			[number of threads] -t --threads=[THREADS] 'The number of resource pack files to process in parallel, in different threads. By default, a value appropriate to the CPU the program is running on is used, but you might want to decrease it to reduce memory usage'
			[ZIP obfuscation] -o --zip-obfuscation 'If provided, the generated ZIP file will not be able to be read by some common programs, like WinRAR, 7-Zip or unzip, but will function normally within Minecraft. It will also not duplicate some metadata in the ZIP file, reducing its size a bit more. You shouldn't rely on this feature to keep the resources safe from unauthorized ripping, and there is a low possibility that a future version of Minecraft rejects these ZIP files. Therefore, it is disabled by default'
			[Compress already compressed files] -c --compress-compressed 'If specified, resource files that are already compressed by design (like PNG and OGG files) will be losslessly compressed again in the result ZIP file, unless that yields no significant space savings. This is disabled by default, as it is expected that compressing already compressed file formats yields marginal space savings that do not outweigh the time cost'
			<resource pack directory> 'The directory where the resource pack to process is'
			[result ZIP file] 'The path to the resulting ZIP file, ready to be distributed'
		").get_matches_from(env::args());

	if let Err(error) = run(parameters) {
		eprintln!(
			"An error occurred while performing the requested operations: {}",
			error
		);
		process::exit(1);
	}
}

fn run(parameters: ArgMatches) -> Result<(), Box<dyn Error>> {
	let mut progress = MultiBar::new();

	let skip_pack_icon = parameters.is_present("skip pack icon");
	let use_zip_obfuscation = parameters.is_present("ZIP obfuscation");
	let compress_already_compressed = parameters.is_present("Compress already compressed files");
	let threads = cmp::max(
		match parameters.value_of("number of threads") {
			Some(threads_string) => FromStr::from_str(threads_string),
			None => Ok(num_cpus::get() * 2)
		}?,
		1
	);
	let output_file_name = parameters
		.value_of("result ZIP file")
		.unwrap_or("resource_pack.zip");
	let canonical_root_path =
		&PathBuf::from(parameters.value_of("resource pack directory").unwrap()).canonicalize()?;

	let file_count = Arc::new(AtomicUsize::new(0));
	let file_process_thread_pool = ThreadPool::new(threads);
	let micro_zip = Arc::new(MicroZip::new(0, use_zip_obfuscation));

	// Process the entire resource pack directory
	process_directory(
		canonical_root_path,
		canonical_root_path,
		&file_count,
		&file_process_thread_pool,
		&mut progress,
		skip_pack_icon,
		compress_already_compressed,
		&micro_zip
	)?;

	// Listen for progress bar changes and wait until all is done
	progress.listen();

	// Append the central directory
	println!("> Finishing up resource pack ZIP file...");
	micro_zip.finish_and_write(&mut fs::File::create(output_file_name)?)?;

	println!(
		"{} processed resource pack files were saved at {}.",
		file_count.load(Ordering::Relaxed),
		output_file_name
	);

	Ok(())
}

/// Recursively processes all the resource pack files in the given path,
/// storing the resulting processed resource pack file data in a vector.
#[allow(clippy::too_many_arguments)] // Not really much point in grouping the arguments
fn process_directory<T: Write>(
	canonical_root_path: &PathBuf,
	current_path: &PathBuf,
	file_count: &Arc<AtomicUsize>,
	thread_pool: &ThreadPool,
	progress: &mut MultiBar<T>,
	skip_pack_icon: bool,
	compress_already_compressed: bool,
	micro_zip: &Arc<MicroZip>
) -> Result<(), Box<dyn Error>> {
	for entry in fs::read_dir(current_path)? {
		let entry = entry?;
		let path = entry.path().canonicalize()?;
		let file_type = entry.file_type()?;

		let is_directory = file_type.is_dir();
		let is_file = file_type.is_file();

		if is_directory || is_file {
			if is_directory {
				process_directory(
					canonical_root_path,
					&path,
					file_count,
					&thread_pool,
					progress,
					skip_pack_icon,
					compress_already_compressed,
					micro_zip
				)?;
			} else {
				let mut relative_path = relativize_path_for_zip_file(&canonical_root_path, &path)?;
				let relative_path_str = match relative_path.to_str() {
					Some(path) => String::from(path),
					None => {
						return Err(Box::new(SimpleError::new(
							"The path contains invalid UTF-8 codepoints"
						)))
					}
				};

				let path_in_root = path.parent().unwrap() == canonical_root_path;

				let mut file_progress = progress.create_bar(0);
				file_progress.message(format!("> {}... ", relative_path_str).as_str());
				file_progress.show_tick = true;
				file_progress.show_speed = false;
				file_progress.show_percent = false;
				file_progress.show_counter = false;
				file_progress.tick();

				let file_count = file_count.clone();
				let micro_zip = micro_zip.clone();

				thread_pool.execute(move || {
					if let Some(resource_pack_file) = resource_pack_file::path_to_resource_pack_file(
						&path,
						skip_pack_icon,
						path_in_root
					) {
						let result = resource_pack_file.process(&mut file_progress);

						if result.is_ok() {
							let (processed_bytes, message) = result.ok().unwrap();

							// Change the relative path with the canonical extension
							relative_path.set_extension(resource_pack_file.canonical_extension());

							// Add it to the ZIP file
							let add_result = micro_zip.add_file(
								&relative_path,
								ZipFileType::RegularFile,
								&processed_bytes,
								resource_pack_file.is_compressed() && !compress_already_compressed,
								&mut file_progress
							);

							if add_result.is_ok() {
								file_progress.finish_println(&format!(
									"> {}: {}",
									relative_path_str, message
								));

								file_count.fetch_add(1, Ordering::Relaxed);
							} else {
								file_progress.finish_println(&format!(
									"> {}: Error occurred while adding to the ZIP file: {}",
									relative_path_str,
									add_result.err().unwrap()
								));
							}
						} else {
							file_progress.finish_println(&format!(
								"> {}: Error occurred while processing: {}",
								relative_path_str,
								result.err().unwrap()
							));
						}
					} else {
						file_progress.finish_println(&format!("> {}: Skipped", relative_path_str));
					}
				});
			}
		}
	}

	Ok(())
}

/// Relativizes the specified path in a platform-independent format from a given root path.
/// The resulting path is appropriate for using in ZIP files structures.
/// This function assumes that symlinks are not relativized.
fn relativize_path_for_zip_file(
	canonical_root_path: &PathBuf,
	canonical_descendant_path: &PathBuf
) -> Result<PathBuf, Box<dyn Error>> {
	let root_components = Vec::from_iter(canonical_root_path.components());
	let mut relativized_path = PathBuf::new();

	for (i, descendant_component) in canonical_descendant_path.components().enumerate() {
		if i < root_components.len() && root_components[i] == descendant_component {
			// If they are the same component, we are still in the components
			// that are also common with the root
		} else {
			// This component is a descendant of the root, store it
			relativized_path.push(PathBuf::from(descendant_component.as_os_str()));
		}
	}

	Ok(relativized_path)
}
