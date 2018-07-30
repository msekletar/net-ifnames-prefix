// SPDX-License-Identifier:  MIT

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate hwaddr;
extern crate libudev;
extern crate ini;
extern crate regex;
extern crate libc;

mod config;
mod sema;

use std::error::Error;
use regex::Regex;
use std::fs::File;
use std::io::prelude::*;

use sema::*;
use config::*;

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

fn exit_maybe_unlock(sema: Option<&mut Semaphore>, exit_code: i32) {
    if let Some(s) = sema {
        s.unlock();
    }

    std::process::exit(exit_code);
}

fn main() {
    env_logger::init();

    // XXX: Figure out how to get rid of the temporary
    let p = get_prefix_from_file("/proc/cmdline");
    if p.is_err() {
        // XXX: We are using {:?} because there is no Display trait for Result
        // It would be better to use {} to make error message nicer but we can't
        // call p.unwrap_err() since that moves the value and borrow checker complains
        // that we are using moved value when assigning to prefix
        error!("Failed to obtain prefix value: {:?}", p);
        exit_maybe_unlock(None, 1);
    }
    let prefix = p.unwrap();

    if prefix.is_empty() {
        info!("No prefix specified on the kernel command line");
        exit_maybe_unlock(None, 0);
    }

    let s = Semaphore::new_with_name("net-prefix-ifnames");
    if s.is_err() {
        error!("Failed to initialize semaphore: {:?}", s);
        exit_maybe_unlock(None, 1);
    }
    let mut sema = s.unwrap();

    sema.lock();

    let mut config = NetSetupLinkConfig::new_with_prefix(&prefix);
    if let Err(e) = config.load() {
        error!("Failed to load current state of network links: {}", e);
        exit_maybe_unlock(Some(&mut sema), 1);
    }

    let d = LinkConfig::hwaddr_from_event_device();
    if d.is_err() {
        error!("Failed to determine MAC address for the event device: {:?}", d);
        exit_maybe_unlock(Some(&mut sema), 1);
    };
    let event_device_hwaddr = d.unwrap();

    if let Some(_c) = config.for_hwaddr(&event_device_hwaddr) {
        info!("Found net_setup_link config for the event device, not generating new one");
        exit_maybe_unlock(Some(&mut sema), 0);
    }

    let n = config.next_link_name();
    if n.is_err() {
        error!("Failed to create new name for the link: {:?}", n);
        exit_maybe_unlock(Some(&mut sema), 1);
    }
    let next_link_name = n.unwrap();


    let lc = LinkConfig::new_with_hwaddr(&next_link_name, &event_device_hwaddr);
    if lc.is_err() {
        error!("Failed to create link config object: {:?}", lc);
        exit_maybe_unlock(Some(&mut sema), 1);
    }
    let link_config = lc.unwrap();

    if let Err(e) = link_config.write_link_file() {
        error!("Failed to write link file for {}: {}", link_config.name, e);
        exit_maybe_unlock(Some(&mut sema), 1);
    }

    info!("New link file was generated at {}", link_config.link_file_path().into_os_string().into_string().unwrap());
    info!("Consider rebuilding initrd image, using \"dracut -f\"");

    sema.unlock();
}
