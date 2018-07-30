// SPDX-License-Identifier:  MIT

use std::fs::File;
use std::fs::read_dir;
use std::error::Error;
use std::io::{Write};
use std::string::ToString;
use std::cmp::Ordering;
use std::path::PathBuf;
use std::env;
use std::collections::HashMap;

use hwaddr::HwAddr;
use ini::Ini;
use libudev;

static NET_SETUP_LINK_CONF_DIR : &'static str = "/etc/systemd/network/";
static LINK_FILE_PREFIX : &'static str = "70-net-ifnames-prefix-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkConfig {
    pub name: String,
    pub hwaddr: HwAddr
}

impl LinkConfig {
    pub fn new<T: ToString>(link_name: &T) -> Result<LinkConfig, Box<Error>> {
        let config = LinkConfig {
            name: link_name.to_string(),
            hwaddr: LinkConfig::hwaddr_from_event_device()?
        };

        Ok(config)
    }

    pub fn new_with_hwaddr<T: ToString>(link_name: &T, hwaddr: &HwAddr) -> Result<LinkConfig, Box<Error>> {
        let config = LinkConfig {
            name: link_name.to_string(),
            hwaddr: *hwaddr,
        };

        Ok(config)
    }

    pub fn hwaddr_from_event_device() -> Result<HwAddr, Box<Error>> {
        let udev = libudev::Context::new()?;
        let devpath = env::var("DEVPATH")?;
        let mut syspath = "/sys".to_string();

        syspath.push_str(&devpath);

        let mac = udev.device_from_syspath(&PathBuf::from(syspath))?.attribute_value("address").ok_or("Failed to get MAC Address")?.to_owned();
        let mac: &str = mac.to_str().ok_or("Failed to convert OsStr to String")?;
        let hwaddr = mac.parse::<HwAddr>()?;

        Ok(hwaddr)
    }

    pub fn link_file_path(&self) -> PathBuf {
        let mut path = PathBuf::from(NET_SETUP_LINK_CONF_DIR);

        path.push(LINK_FILE_PREFIX.to_string() + &self.name + ".link");
        path
    }

    pub fn write_link_file(&self) -> Result<(), Box<Error>> {
        let path = self.link_file_path();
        debug!("{:?}", path);
        let mut link_file = File::create(path)?;
        let mac = LinkConfig::hwaddr_from_event_device()?;

        write!(&mut link_file, "[Match]\nMACAddress={}\n\n[Link]\nName={}\n", mac, self.name)?;

        Ok(())
    }
}

impl Ord for LinkConfig {
    fn cmp(&self, other: &LinkConfig) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for LinkConfig {
    fn partial_cmp(&self, other: &LinkConfig) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct NetSetupLinkConfig  {
    config: HashMap<HwAddr, LinkConfig>,
    links: Vec<LinkConfig>,
    ifname_prefix: String
}

impl NetSetupLinkConfig {
    pub fn new_with_prefix(prefix: &String) -> Self {
        NetSetupLinkConfig {
            config: HashMap::new(),
            links: Vec::new(),
            ifname_prefix: prefix.clone()
        }
    }

    pub fn load(&mut self) -> Result<(), Box<Error>> {
        self.enumerate_links_from_udev()?;
        self.enumerate_links_from_files()?;

        // Most links have link file present and are currently known to udev.
        // Hence enumeration from both sources created duplicate entries in the links vector.
        self.links.sort();
        self.links.dedup();

        Ok(())
    }

    pub fn for_hwaddr(&self, mac: &HwAddr) -> Option<LinkConfig> {
        if let Some(c) = self.config.get(mac) {
            return Some(c.clone());
        }
        None
    }

    pub fn next_link_name(&self) -> Result<String, Box<Error>> {
        if self.links.is_empty() {
            return Ok(format!("{}{}", self.ifname_prefix, "0"));
        }

        let last = self.links.last().ok_or("Failed to obtain last vector element")?;
        let last_index = last.name.trim_left_matches(&self.ifname_prefix).parse::<u64>()?;

        Ok(format!("{}{}", self.ifname_prefix, &(last_index + 1).to_string()))
    }

    fn match_ethernet_links(udev_enumerate: &mut libudev::Enumerator) -> Result<(), Box<Error>> {
        udev_enumerate.match_subsystem("net")?;
        udev_enumerate.match_attribute("type", "1")?;

        Ok(())
    }

    fn enumerate_links_from_udev(&mut self) -> Result<(), Box<Error>> {
        let udev  = libudev::Context::new()?;
        let mut enumerate = libudev::Enumerator::new(&udev)?;
        let mut links = Vec::new();

        NetSetupLinkConfig::match_ethernet_links(&mut enumerate)?;

        for device in enumerate.scan_devices()? {
            let link = String::from(device.sysname().to_str().ok_or("Failed to convert device sysname (OsStr) to string slice")?);
            links.push(LinkConfig::new(&link)?);
        }

        links = links.iter()
            .filter_map(|l| if l.name.starts_with(&self.ifname_prefix) { Some(l) } else { None })
            .cloned()
            .collect();

        self.links = links;

        Ok(())
    }

    fn enumerate_links_from_files(&mut self) -> Result<(), Box<Error>> {
        let mut link_files = Vec::new();

        for n in read_dir("/etc/systemd/network")? {
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

            let mac = match_section.get("MACAddress").ok_or("Failed to parse link file, \"MACAddress\"' option not present in the [Link] section")?;
            let name = link_section.get("Name").ok_or("Failed to parse link file, \"Name\" option not present in the [Link] section")?;

            if ! name.starts_with(&self.ifname_prefix) {
                warn!("Unexpected link name");
                continue;
            }

            let hwaddr = mac.parse::<HwAddr>()?;

            self.config.insert(hwaddr, LinkConfig::new(name)?);
            self.links.push(LinkConfig::new(name)?);
        }
        Ok(())
    }
}
