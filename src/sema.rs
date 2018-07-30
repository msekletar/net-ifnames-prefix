use std::error::Error;
use std::ffi::CString;
use std::process::id;

use libc;

#[derive(Debug)]
pub struct Semaphore {
    raw_sema: *mut libc::sem_t,
}

impl Semaphore {
    pub fn new_with_name(name: &str) -> Result<Semaphore, Box<Error>> {
        let raw_sema_name = CString::new(name)?;

        let s;
        unsafe {
            s = libc::sem_open(raw_sema_name.as_ptr() as *const i8, libc::O_CREAT, libc::S_IRUSR | libc::S_IWUSR, 1);
            if s.is_null() {
                return Err(From::from("Failed to allocate named semaphore, sem_open() failed"));
            }
        }
        
        Ok(Semaphore{raw_sema: s})
    }

    pub fn lock(&mut self) {
        unsafe {
            libc::sem_wait(self.raw_sema);
            debug!("lock taken by PID={}", id());
        }

    }
    
    pub fn unlock(&mut self) {
        unsafe {
            debug!("lock released by PID={}", id());
            libc::sem_post(self.raw_sema);
        }
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        self.unlock();
        
        unsafe {
            libc::sem_close(self.raw_sema);
        }
    }
}
