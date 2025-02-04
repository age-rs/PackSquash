//! OS-specific functions to get PackSquash system IDs.

use super::SystemId;

#[cfg(all(unix, not(target_os = "macos")))]
use std::{fs, io, path::Path};

/// Gets the D-Bus and/or systemd generated machine ID. This machine ID is
/// 128-bit wide, and is intended to be constant for all the lifecycle of the
/// OS install, no matter if hardware is replaced or some configuration is
/// changed.
///
/// Although originally Linux-specific, D-Bus can be run in BSD derivatives,
/// and Linux is pretty influential in the Unix world, so it's worth trying
/// on most Unix-like systems.
///
/// Further reading:
/// - <https://www.freedesktop.org/software/systemd/man/machine-id.html>
/// - <https://unix.stackexchange.com/questions/396052/missing-etc-machine-id-on-freebsd-trueos-dragonfly-bsd-et-al>
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
pub(super) fn get_dbus_machine_id() -> Option<SystemId> {
	u128::from_str_radix(
		read_uuid_file("/etc/machine-id")
			.or_else(|_| read_uuid_file("/var/lib/dbus/machine-id"))
			.or_else(|_| read_uuid_file("/var/db/dbus/machine-id"))
			.or_else(|_| read_uuid_file("/usr/local/etc/machine-id"))
			.or_else(|_| read_uuid_file("/run/machine-id"))
			.ok()?
			.trim(),
		16
	)
	.ok()
	.map(|id| SystemId::new(id, false))
}

/// Gets the ID generated by the Linux kernel for the current boot. Although
/// it has the desirable properties of being 128-bit wide, userspace-agnostic,
/// and available in virtually every Linux kernel (at least from 2.2), it
/// changes in each boot, so it's pretty volatile. It relies on sysctl and
/// procfs being mounted at /proc.
///
/// Further reading:
/// - <https://www.kernel.org/doc/html/latest/admin-guide/sysctl/kernel.html#random>
/// - <http://0pointer.de/blog/projects/ids.html>
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) fn get_boot_id() -> Option<SystemId> {
	use uuid::Uuid;

	Some(SystemId::new(
		read_uuid_file("/proc/sys/kernel/random/boot_id")
			.ok()?
			.trim()
			.parse::<Uuid>()
			.ok()?
			.into_bytes(),
		true
	))
}

/// Gets the host ID generated by a BSD kernel via sysctl system calls. Although
/// its precise generation method is not very well documented, it usually gives a
/// D-Bus-like machine ID.
///
/// Further reading:
/// - <https://www.freebsd.org/cgi/man.cgi?query=sysctl&apropos=0&sektion=0&manpath=FreeBSD+14.0-current&arch=default&format=html>
/// - <https://unix.stackexchange.com/questions/396052/missing-etc-machine-id-on-freebsd-trueos-dragonfly-bsd-et-al>
/// - <https://github.com/netdata/netdata/issues/2682#issuecomment-327721829>
#[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
pub(super) fn get_kernel_host_id() -> Option<SystemId> {
	use std::ffi::{CStr, CString};
	use std::os::raw::{c_char, c_int, c_void};
	use std::ptr;
	use uuid::Uuid;

	#[allow(unsafe_code)] // SAFETY: the system call definition is correct
	unsafe extern "C" {
		/// `int sysctlbyname(const char* name, void* oldp, size_t* oldlenp, void* newp, size_t newlen)`, from `#include <sys/sysctl.h>`.
		///
		/// Documentation: <https://www.freebsd.org/cgi/man.cgi?query=sysctlbyname&apropos=0&sektion=0&manpath=FreeBSD+14.0-current&arch=default&format=html>
		fn sysctlbyname(
			name: *const c_char,
			oldp: *mut c_void,
			oldlenp: *mut usize,
			newp: *const c_void,
			newlen: usize
		) -> c_int;
	}

	let kernel_host_uuid_key = CString::new("kern.hostuuid").unwrap();

	/// Size of the biggest standard UUID format, with dashes.
	const BUFFER_SIZE: usize = 36 + 1;
	let mut buf: [c_char; BUFFER_SIZE] = [0; BUFFER_SIZE];
	let mut buffer_size = BUFFER_SIZE;

	#[allow(unsafe_code)]
	match unsafe {
		sysctlbyname(
			kernel_host_uuid_key.as_ptr(),
			buf.as_mut_ptr() as *mut c_void,
			&mut buffer_size,
			ptr::null(),
			0
		)
	} {
		0 => Some(SystemId::new(
			Uuid::parse_str(
				unsafe { CStr::from_ptr(&buf as *const c_char) }
					.to_str()
					.ok()?
			)
			.ok()?
			.into_bytes(),
			false
		)),
		// An error occurred
		_ => None
	}
}

/// Gets the product UUID from the Desktop Management Interface, which is BIOS-provided,
/// and is usually linked with the motherboard. This UUID is extremely persistent, even
/// across operating systems, but it may not be defined on all platforms, and sometimes
/// it is initialized to dummy values.
///
/// Further reading:
/// - <https://lists.freebsd.org/pipermail/freebsd-hackers/2007-February/019456.html>
#[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
pub(super) fn get_dmi_product_id() -> Option<SystemId> {
	use std::ffi::{CStr, CString};
	use std::os::raw::{c_char, c_int};
	use uuid::Uuid;

	#[allow(unsafe_code)] // SAFETY: the system call definition is correct
	unsafe extern "C" {
		/// `int kenv(int action, const char* name, char* value, int len)`, from `#include <kenv.h>`.
		///
		/// Documentation: <https://www.freebsd.org/cgi/man.cgi?query=kenv&sektion=2&format=html>
		fn kenv(action: c_int, name: *const c_char, value: *mut c_char, len: c_int) -> c_int;
	}

	/// Constant for the KENV_GET action.
	///
	/// Source: <http://fxr.watson.org/fxr/source/sys/kenv.h?im=10>
	const KENV_GET: c_int = 0;

	/// Maximum value length.
	///
	/// Source: <http://fxr.watson.org/fxr/source/sys/kenv.h?im=10>
	const KENV_MVALLEN: usize = 128;

	let dmi_product_uuid_key = CString::new("smbios.system.uuid").unwrap();
	let mut buf = [0; KENV_MVALLEN + 1];

	#[allow(unsafe_code)]
	match unsafe {
		kenv(
			KENV_GET,
			dmi_product_uuid_key.as_ptr(),
			buf.as_mut_ptr(),
			buf.len() as c_int
		)
	} {
		// If 32 or 36 characters were written (+1 NUL terminator), it may be
		// a UUID, so try to parse it
		33 | 37 => Some(SystemId::new(
			Uuid::parse_str(
				unsafe { CStr::from_ptr(&buf as *const c_char) }
					.to_str()
					.ok()?
			)
			.ok()?
			.into_bytes(),
			false
		)),
		// An error occurred, or we got something that is not a UUID
		_ => None
	}
}

/// Gets the product UUID reported by the Desktop Management Interface, which is BIOS-provided,
/// and is usually linked with the motherboard, from sysfs. This UUID is extremely persistent,
/// even across operating systems, but it may not be defined on all platforms, sometimes it
/// is initialized to dummy values, and reading it requires root privileges.
///
/// Further reading:
/// - <https://lists.freebsd.org/pipermail/freebsd-hackers/2007-February/019456.html>c
/// - <https://utcc.utoronto.ca/~cks/space/blog/linux/DMIDataInSysfs>
/// - <https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/drivers/firmware/dmi-id.c>
#[cfg(target_os = "linux")]
pub(super) fn get_dmi_product_id() -> Option<SystemId> {
	use uuid::Uuid;

	read_uuid_file("/sys/class/dmi/id/product_uuid")
		.ok()?
		.trim()
		.parse::<Uuid>()
		.ok()
		.map(|uuid| SystemId::new(uuid.into_bytes(), false))
}

/// Gets a system identifier that aggregates the DMI serial numbers collected by
/// the udev database, provided by the BIOS. Unlike directly reading the DMI product
/// ID from sysfs, this method does not require root privileges, and takes into account
/// more serial numbers, but assumes that a suitable udev daemon with a compatible
/// database format is running. On modern Linux distributions, this is usually implemented
/// by systemd-udevd.
///
/// Further reading:
/// - <https://man7.org/linux/man-pages/man7/udev.7.html>
/// - <https://www.phoronix.com/news/Linux-DIMM-Details-As-Root>
/// - <https://man7.org/linux/man-pages/man8/systemd-udevd.service.8.html>
#[cfg(target_os = "linux")]
pub(super) fn get_aggregated_dmi_serial_numbers_id() -> Option<SystemId> {
	use sha2::{
		Digest, Sha224,
		digest::{OutputSizeUser, typenum::Unsigned}
	};
	use std::collections::BinaryHeap;
	use std::fs::File;
	use std::io::{BufRead, BufReader, Read};

	let mut aggregated_serials = BinaryHeap::new();

	for udev_dmi_db_line in BufReader::new(File::open("/run/udev/data/+dmi:id").ok()?)
		.take(16384) // Defensively handle files with too long lines
		.lines()
	{
		let mut udev_db_line = udev_dmi_db_line.ok()?;

		// See https://www.man7.org/linux/man-pages/man8/udevadm.8.html, Table 1, for
		// docs. E means that this is a device property entry
		let Some((_entry_prefix @ "E", attribute_name_and_value)) = udev_db_line.split_once(':')
		else {
			continue;
		};

		let Some((attribute_name, attribute_value)) = attribute_name_and_value.split_once('=') else {
			continue;
		};

		if !attribute_name.ends_with("_SERIAL_NUMBER") {
			continue;
		}

		// In-place truncation of the udev_db_line string to only hold the attribute value.
		// This works because attribute_value points to a substring within the udev_db_line string
		// allocation, and ranges are expressed in byte indices
		udev_db_line.drain(0..attribute_value.as_ptr() as usize - udev_db_line.as_ptr() as usize);
		let attribute_value = udev_db_line;

		aggregated_serials.push(attribute_value);
	}

	// The ordering of the serial numbers may vary between invocations due to firmware quirks,
	// udev changes, and physical tampering, so sort them before fingerprinting to reduce
	// volatility. Doing heapsort through a binary heap instead of a plain Vec and introsort has
	// the advantage of lower code size overhead for smaller collections and better interleaving
	// of the per-element sorting and hashing operations, which may lead to better codegen here
	let mut serial_numbers_digest = Sha224::new();
	while let Some(serial_number) = aggregated_serials.pop() {
		serial_numbers_digest.update(serial_number);
	}

	Some(SystemId::new(
		<[u8; <Sha224 as OutputSizeUser>::OutputSize::USIZE]>::from(serial_numbers_digest.finalize()),
		false
	))
}

/// Uses the Core Foundation and IOKit frameworks to get the `IOPlatformSerialNumber`
/// property of the `IOPlatformExpertDevice` service to get a machine ID. This represents
/// a unique serial number, somewhat similar to the DMI product UUID in conventional PCs.
/// Under rare circumstances, however, this ID may not be available. Also, its format is
/// not specified.
///
/// Further reading:
/// - <http://mirror.informatimago.com/next/developer.apple.com/technotes/tn/tn1103.html>
/// - <https://developer.apple.com/documentation/corefoundation/>
/// - <https://developer.apple.com/documentation/iokit/>
/// - <https://docs.rs/io-kit-sys/0.1.0/io_kit_sys/>
/// - <https://svartalf.info/posts/2019-05-31-poking-the-macos-io-kit-with-rust>
/// - <https://github.com/svartalf/rust-battery/blob/20233871e16b0e7083281df560875110a0cac93b/battery/src/platform/darwin/iokit/sys.rs>
/// - <https://github.com/servo/core-foundation-rs/blob/master/core-foundation>
#[cfg(target_os = "macos")]
#[allow(unsafe_code, non_camel_case_types)]
pub(super) fn get_platform_serial_number() -> Option<SystemId> {
	use core_foundation::{
		base::{CFAllocatorRef, CFTypeRef, TCFType, kCFAllocatorDefault, mach_port_t},
		dictionary::{CFDictionaryRef, CFMutableDictionaryRef},
		string::{CFString, CFStringRef}
	};
	use mach2::kern_return::kern_return_t;
	use sha2::{
		Digest, Sha224,
		digest::{OutputSizeUser, typenum::Unsigned}
	};
	use std::{ffi::CString, os::raw::c_char};

	type io_object_t = mach_port_t;
	type io_registry_entry_t = io_object_t;
	type io_service_t = io_object_t;
	type IOOptionBits = u32;

	#[allow(unsafe_code)] // SAFETY: the system call definition is correct
	#[link(name = "IOKit", kind = "framework")]
	unsafe extern "C" {
		static kIOMasterPortDefault: mach_port_t;

		/// Documentation: <https://developer.apple.com/documentation/iokit/1514687-ioservicematching?language=objc>
		fn IOServiceMatching(name: *const c_char) -> CFMutableDictionaryRef;

		/// Documentation: <https://developer.apple.com/documentation/iokit/1514535-ioservicegetmatchingservice?language=objc>
		fn IOServiceGetMatchingService(
			masterPort: mach_port_t,
			matching: CFDictionaryRef
		) -> io_service_t;

		/// Documentation: https://developer.apple.com/documentation/iokit/1514293-ioregistryentrycreatecfproperty?language=objc
		fn IORegistryEntryCreateCFProperty(
			entry: io_registry_entry_t,
			key: CFStringRef,
			allocator: CFAllocatorRef,
			options: IOOptionBits
		) -> CFTypeRef;

		/// Documentation: <https://developer.apple.com/documentation/iokit/1514627-ioobjectrelease?language=objc>
		fn IOObjectRelease(object: io_object_t) -> kern_return_t;
	}

	let expert_device_service = unsafe {
		let service_name = CString::new("IOPlatformExpertDevice").unwrap();
		IOServiceGetMatchingService(
			kIOMasterPortDefault,
			IOServiceMatching(service_name.as_ptr())
		)
	};
	if expert_device_service == 0 {
		return None;
	}

	let release_objects = || unsafe { IOObjectRelease(expert_device_service) };

	let serial_number_cf_string_ref = unsafe {
		IORegistryEntryCreateCFProperty(
			expert_device_service,
			CFString::from_static_string("IOPlatformSerialNumber").as_concrete_TypeRef(),
			kCFAllocatorDefault,
			0
		)
	};
	if serial_number_cf_string_ref == 0 as CFTypeRef {
		release_objects();
		return None;
	}

	// Apple "thinks different", so the format of this serial number is not specified,
	// because they are not happy with just making you pay exorbitant prices. Assume
	// it may be any string
	let serial_number_string =
		unsafe { CFString::wrap_under_create_rule(serial_number_cf_string_ref as CFStringRef) }
			.to_string();

	let result = SystemId::new(
		<[u8; <Sha224 as OutputSizeUser>::OutputSize::USIZE]>::from(Sha224::digest(
			serial_number_string
		)),
		false
	);

	release_objects();

	Some(result)
}

/// Uses the POSIX `gethostid` system call to get a host identifier. Although portable,
/// and usually (but not necessarily) persistent across boots, the returned ID is only
/// 32-bits long, and the exact way this ID is generated is system-dependent. It is
/// usually generated from a proper machine UUID in some BSDs (best case scenario), from
/// the network configuration, read from /etc/hostid, or even just return a constant value
/// (worst case scenario, happens on musl and allegedly sometimes on macOS).
///
/// Further reading:
/// - <https://pubs.opengroup.org/onlinepubs/9699919799/functions/gethostid.html>
/// - <https://man7.org/linux/man-pages/man3/gethostid.3.html>
/// - <https://www.freebsd.org/cgi/man.cgi?query=gethostid&sektion=3&apropos=0&manpath=freebsd>
/// - <https://docs.oracle.com/cd/E86824_01/html/E54766/gethostid-3c.html>
/// - <https://git.musl-libc.org/cgit/musl/tree/src/misc/gethostid.c>
/// - <https://bug-coreutils.gnu.narkive.com/4cnKKtfD/workaround-for-hostid-on-darwin-8-macppc>
#[cfg(unix)]
pub(super) fn get_host_id() -> Option<SystemId> {
	use std::os::raw::c_long;

	#[allow(unsafe_code)] // SAFETY: the system call definition is correct
	unsafe extern "C" {
		/// `long gethostid()`, from `#include <unistd.h>`.
		///
		/// Documentation: <https://pubs.opengroup.org/onlinepubs/9699919799/functions/gethostid.html>
		fn gethostid() -> c_long;
	}

	#[allow(unsafe_code)]
	Some(SystemId::new(unsafe { gethostid() }, true))
}

/// Gets a machine ID, persistent across upgrades, from its most common location in the
/// Windows registry. This key is not documented in official Microsoft sources, but shows
/// up a lot for this purpose over the Internet and is pretty reliable, even working under
/// Wine.
#[cfg(windows)]
pub(super) fn get_machine_id() -> Option<SystemId> {
	use uuid::Uuid;
	use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

	let machine_guid: String = RegKey::predef(HKEY_LOCAL_MACHINE)
		.open_subkey("SOFTWARE\\Microsoft\\Cryptography")
		.ok()?
		.get_value("MachineGuid")
		.ok()?;

	Some(SystemId::new(
		Uuid::parse_str(&machine_guid).ok()?.into_bytes(),
		false
	))
}

/// Uses Windows Management Interface to get a product UUID. SMBIOS DMI is used to get this UUID.
/// It may be all zeros if a product UUID is not available.
///
/// Further reading:
/// - <https://docs.microsoft.com/en-us/windows/win32/cimwin32prov/win32-computersystemproduct>
#[cfg(windows)]
pub(super) fn get_dmi_product_id() -> Option<SystemId> {
	use serde::Deserialize;
	use uuid::Uuid;
	use wmi::{COMLibrary, WMIConnection};

	#[derive(Deserialize)]
	#[serde(rename = "Win32_ComputerSystemProduct")]
	struct ComputerSystemProduct {
		#[serde(rename = "UUID")]
		uuid: String
	}

	let product_info: ComputerSystemProduct =
		WMIConnection::with_namespace_path("ROOT\\CIMV2", COMLibrary::new().ok()?.into())
			.ok()?
			.get()
			.ok()?;

	Some(SystemId::new(
		Uuid::parse_str(&product_info.uuid).ok()?.into_bytes(),
		false
	))
}

/// Returns the serial number of the filesystem volume that contains the Windows system root
/// directory, which is usually accessible at `C:\Windows`.
///
/// The Windows system root directory is robustly and efficiently resolved by using a well-known
/// UNC path with a Win32 API namespace selector that points to the `SystemRoot` object provided
/// by the [Windows Executive] [Object Manager] subsystem at the root namespace, which itself
/// is a symlink to the actual system root directory under a disk device.
///
/// Further reading:
/// - <https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-dtyp/62e862f4-2a51-452e-8eeb-dc4ff5ee33cc>
/// - <https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file>
/// - <https://stackoverflow.com/questions/25090101/is-there-a-difference-between-and-paths>
/// - <https://googleprojectzero.blogspot.com/2016/02/the-definitive-guide-on-win32-to-nt.html>
///
/// [Windows Executive]: https://en.wikipedia.org/wiki/Architecture_of_Windows_NT#Executive
/// [Object Manager]: https://en.wikipedia.org/wiki/Object_Manager
#[cfg(windows)]
pub(super) fn get_system_root_volume_id() -> Option<SystemId> {
	use std::{fs, os::windows::fs::MetadataExt};

	fs::metadata(r"\\?\GLOBALROOT\SystemRoot")
		.ok()
		.and_then(|metadata| metadata.volume_serial_number())
		.map(|volume_serial_number| SystemId::new(volume_serial_number, false))
}

/// Returns the Windows install date. This install date may be changed after some updates
/// and, because it is 32-bit long, it is pretty weak as cryptographic material.
#[cfg(windows)]
pub(super) fn get_install_date() -> Option<SystemId> {
	use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

	let install_date: u32 = RegKey::predef(HKEY_LOCAL_MACHINE)
		.open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
		.ok()?
		.get_value("InstallDate")
		.ok()?;

	Some(SystemId::new(
		install_date,
		true // Murphy's law corollary: Windows will update itself when it's a bad time
	))
}

#[cfg(all(unix, not(target_os = "macos")))]
/// Reads a file that is expected to contain a UUID in text format to a string.
/// Differently from other helper methods available in the standard library,
/// like `read_to_string`, this limits the maximum number of bytes read,
/// so we discard invalid big files pretty fast and while consuming little
/// memory.
fn read_uuid_file(path: impl AsRef<Path>) -> io::Result<String> {
	use std::io::Read;

	/// The maximum size of a UUID, assuming its hyphenated representation.
	const UUID_LENGTH: usize = 36;

	let mut buf = [0; UUID_LENGTH];
	let mut file = fs::File::open(path)?;
	let mut i = 0;

	// Read file bytes until we fill the buffer or reach EOF
	while i < UUID_LENGTH {
		let bytes_read = file.read(&mut buf[i..UUID_LENGTH])?;
		i += bytes_read;

		if bytes_read == 0 {
			break;
		}
	}

	Ok(String::from_utf8_lossy(&buf[..i]).into_owned())
}
