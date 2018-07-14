#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate hwaddr;
extern crate libudev;
extern crate regex;
extern crate ini;
extern crate libc;
#[macro_use] extern crate log;
extern crate simple_logger;

use std::fs::File;
use std::fs::read_dir;
use std::error::Error;
use std::io::{Read, Write};
use std::string::ToString;
use std::cmp::Ordering;
use std::iter::Extend;
use std::ffi::CString;
use std::process::id;
use std::path::PathBuf;
use std::env;
use hwaddr::HwAddr;
use log::Level;

use regex::Regex;
use ini::Ini;

static NET_SETUP_LINK_CONF_DIR : &'static str = "/etc/systemd/network/";
static LINK_FILE_PREFIX : &'static str = "70-net-ifnames-prefix-";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Link {
    name: String,
}

impl Link {
    fn new<T: ToString>(link_name: &T) -> Link {
        Link{name: link_name.to_string()}
    }

    fn link_file_path(&self) -> PathBuf {
        let mut path = PathBuf::from(NET_SETUP_LINK_CONF_DIR);

        path.push(LINK_FILE_PREFIX.to_string() + &self.name + ".link");
        
        path
    }
}

impl Ord for Link {
    fn cmp(&self, other: &Link) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Link {
    fn partial_cmp(&self, other: &Link) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
struct NamedSemaphore {
    raw_sema: *mut libc::sem_t,
}

impl NamedSemaphore {
    fn new(name: &str) -> Result<NamedSemaphore, Box<Error>> {
        let raw_sema_name = CString::new(name)?;

        let s;
        unsafe {
            s = libc::sem_open(raw_sema_name.as_ptr() as *const i8, libc::O_CREAT, libc::S_IRUSR | libc::S_IWUSR, 1);
            if s.is_null() {
                return Err(From::from("Failed to allocate named semaphore, sem_open() failed"));
            }
        }
        
        Ok(NamedSemaphore{raw_sema: s})
    }

    fn lock(&mut self) {
        unsafe {
            libc::sem_wait(self.raw_sema);
            debug!("lock taken by PID={}", id());
        }

    }
    
    fn unlock(&mut self) {
        unsafe {
            debug!("lock released by PID={}", id());
            libc::sem_post(self.raw_sema);
        }
    }
}

impl Drop for NamedSemaphore {
    fn drop(&mut self) {
        self.unlock();
        
        unsafe {
            libc::sem_close(self.raw_sema);
        }
    }
}

fn match_only_ethernet_links(udev_enumerate: &mut libudev::Enumerator) -> Result<(), Box<Error>> {
    udev_enumerate.match_subsystem("net")?;
    udev_enumerate.match_attribute("type", "1")?;

    Ok(())
}

fn get_prefix_from_file(path: &str) -> Result<String, Box<Error>> {
    let mut f = File::open(path)?;
    let mut content = String::new();

    f.read_to_string(&mut content)?;

    let re = Regex::new(r"net.ifnames.prefix=(\w+)")?;
    let captures = re.captures(&content);
    let prefix;
    match captures {
        Some(c) => prefix = c[1].to_string(),
        None => prefix = "".to_string()
    };

    if prefix == "eth" {
        return Err(From::from("Use of prefix \"eth\" is not allowed because it is the prefix used by the kernel"));
    }

    if prefix.len() > 14 {
        return Err(From::from("Prefix too long, maximum length of prefix is 14 characters"));
    }

    Ok(prefix)
}

fn links_enumerate(prefix: &str) -> Result<Vec<Link>, Box<Error>> {
    let udev  = libudev::Context::new()?;
    let mut enumerate = libudev::Enumerator::new(&udev)?;
    let mut links = Vec::new();

    match_only_ethernet_links(&mut enumerate)?;

    for device in enumerate.scan_devices()? {
        let link = String::from(device.sysname().to_str().ok_or("Failed to convert device sysname (OsStr) to string slice")?);
        links.push(Link::new(&link));
    }

    links = links.iter()
        .filter_map(|l| if l.name.starts_with(prefix) { Some(l) } else { None })
        .cloned()
        .collect();

    Ok(links)
}

fn links_enumerate_from_config(prefix: &str) -> Result<Vec<Link>, Box<Error>>    {
    let mut links = Vec::new();
    let network_files = read_dir("/etc/systemd/network")?;
    let mut link_files = Vec::new();

    for n in network_files {
        let entry = match n {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        {
            let name = path.file_name().ok_or("Failed to obtain filename")?.to_str().ok_or("Failed to convert OsStr to String")?;

            if ! name.starts_with(LINK_FILE_PREFIX) || ! name.ends_with(".link") {
                continue;
            }
        }

        link_files.push(path);
    }

    for l in &link_files {
        let conf = Ini::load_from_file(l)?;
        let match_section = conf.section(Some("Match".to_owned())).ok_or("Failed to parse link file, [Match] section not found")?;
        let link_section = conf.section(Some("Link".to_owned())).ok_or("Failed to parse link file, [Link] section not found")?;

        let _mac_addr = match_section.get("MACAddress").ok_or("Failed to parse link file, \"MACAddress\"' option not present in the [Link] section")?;
        let link_name = link_section.get("Name").ok_or("Failed to parse link file, \"Name\" option not present in the [Link] section")?;

        if ! link_name.starts_with(prefix) {
            warn!("Detected unexpected link name");
            continue;
        }

        links.push(Link::new(&link_name));
    }

    Ok(links)
}

fn get_next_link_name(prefix: &str, links: &mut Vec<Link>) -> Result<String, Box<Error>> {

    if links.is_empty() {
        return Ok(String::from(prefix) + "0")
    }

    links.sort();
    links.dedup();

    let last = links.last().ok_or("Failed to obtain last vector element")?;
    let last_index = last.name.trim_left_matches(prefix).parse::<u64>()?;

    Ok(String::from(prefix) + &(last_index + 1).to_string())
}

fn get_mac_from_event_device() -> Result<HwAddr, Box<Error>> {
    let udev = libudev::Context::new()?;
    let devpath = env::var("DEVPATH")?;
    let mut syspath = "/sys".to_string();

    syspath.push_str(&devpath);
        
    let mac = udev.device_from_syspath(&PathBuf::from(syspath))?.attribute_value("address").ok_or("Failed to get MAC Address")?.to_owned();
    let mac: &str = mac.to_str().ok_or("Failed to convert OsStr to String")?;
    let hwaddr = mac.parse::<HwAddr>()?;

    Ok(hwaddr)
}

fn generate_link_file(link: Link) -> Result<(), Box<Error>> {
    let path = link.link_file_path();
    debug!("{:?}", path);
    let mut link_file = File::create(path)?;
    let mac = get_mac_from_event_device()?;

    write!(&mut link_file, "[Match]\nMACAddress={}\n\n[Link]\nName={}\n", mac, link.name)?;
    
    Ok(())
}

fn main() {
    match simple_logger::init_with_level(Level::Warn) {
        Ok(_) => {},
        Err(e) => {
            eprintln!("Failed to initialize logging: {}", e);
            std::process::exit(1);
        }
    }

    let prefix = match get_prefix_from_file("/proc/cmdline") {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to obtain prefix value: {}", e);
            std::process::exit(1);
        }
    };

    if prefix.is_empty() {
        info!("No prefix specified on the kernel command line");
        std::process::exit(0);
    }
        
    let mut sem = match NamedSemaphore::new("net-prefix-ifnames") {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to initialize semaphore: {}", e);
            std::process::exit(1);
        }
    };

    sem.lock();

    let mut links = match links_enumerate(&prefix) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to enumerate links on the system: {}", e);
            sem.unlock();
            std::process::exit(1);
        }
    };
    
    let links_from_config = match links_enumerate_from_config(&prefix) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to enumerate links from the configuration files: {}", e);
            sem.unlock();
            std::process::exit(1);
        }
    };
    
    links.extend(links_from_config);

    let new_link_name = match get_next_link_name(&prefix, &mut links) {
        Ok(n) => n,
        Err(e) => {
            error!("Failed to determine interface name for the event device: {}", e);
            sem.unlock();
            std::process::exit(1);
        }
    };

    let new_link = Link::new(&new_link_name);

    if let Err(e) = generate_link_file(new_link) {
        error!("Failed to generate link file for the event device: {}", e);
        sem.unlock();
        std::process::exit(1);
    }
}    


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn good_prefix() {
        let cmdline = "initrd=initrd root=UUID=0f8838f7-4c18-4db3-a33d-b985a5e25d24 ro rootflags=subvol=f27 LANG=en_US.UTF-8 scsi_mod.use_blk_mq=1 net.ifnames.prefix=net";

        fs::write("test-cmdline", cmdline).unwrap();
        let r = get_prefix_from_file("test-cmdline");
        assert!(r.is_ok());
        fs::remove_file("test-cmdline").unwrap();
    }

    #[test]
    fn bad_prefix_eth() {
        let cmdline = "initrd=initrd root=UUID=0f8838f7-4c18-4db3-a33d-b985a5e25d24 ro rootflags=subvol=f27 LANG=en_US.UTF-8 scsi_mod.use_blk_mq=1 net.ifnames.prefix=eth";

        fs::write("test-cmdline-eth", cmdline).unwrap();
        let r = get_prefix_from_file("test-cmdline-eth");
        assert!(r.is_err());
        fs::remove_file("test-cmdline-eth").unwrap();
    }

    #[test]
    fn no_prefix() {
        let cmdline = "initrd=initrd root=UUID=0f8838f7-4c18-4db3-a33d-b985a5e25d24 ro rootflags=subvol=f27 LANG=en_US.UTF-8 scsi_mod.use_blk_mq=1";

        fs::write("test-cmdline-no-prefix", cmdline).unwrap();
        let r = get_prefix_from_file("test-cmdline-no-prefix");
        assert!(r.is_ok());
        assert!(r.unwrap().is_empty());
        fs::remove_file("test-cmdline-no-prefix").unwrap();
    }

    #[test]
    fn prefix_too_long() {
        let cmdline = "initrd=initrd root=UUID=0f8838f7-4c18-4db3-a33d-b985a5e25d24 ro rootflags=subvol=f27 LANG=en_US.UTF-8 scsi_mod.use_blk_mq=1 net.ifnames.prefix=abcdefghilkjmno";

        fs::write("test-cmdline-long-prefix", cmdline).unwrap();
        let r = get_prefix_from_file("test-cmdline-prefix-long");
        assert!(r.is_err());
        fs::remove_file("test-cmdline-long-prefix").unwrap();
    }

    #[test]
    fn prefix_embedded_in_other_option() {
        let cmdline = "initrd=initrd root=UUID=0f8838f7-4c18-4db3-a33d-b985a5e25d24 ro rootflagsnet.ifnames.prefix=f27 LANG=en_US.UTF-8 scsi_mod.use_blk_mq=1 net.ifnames.prefix=abcdefghilkjmno";

        fs::write("test-cmdline-long-prefix", cmdline).unwrap();
        let r = get_prefix_from_file("test-cmdline-prefix-long");
        assert!(r.is_err());
        fs::remove_file("test-cmdline-long-prefix").unwrap();
    }
}
