use crate::dwarf_data::DwarfData;
use nix::sys::ptrace;
use nix::sys::signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::collections::HashMap;
use std::mem::size_of;
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::process::Command;

fn align_addr_to_word(addr: usize) -> usize {
    addr & (-(size_of::<usize>() as isize) as usize)
}

#[derive(Clone)]
struct Breakpoint {
    addr: usize,
    orig_byte: u8,
}

pub enum Status {
    /// Indicates inferior stopped. Contains the signal that stopped the process, as well as the
    /// current instruction pointer that it is stopped at.
    Stopped(signal::Signal, usize),

    /// Indicates inferior exited normally. Contains the exit status code.
    Exited(i32),

    /// Indicates the inferior exited due to a signal. Contains the signal that killed the
    /// process.
    Signaled(signal::Signal),
}

/// This function calls ptrace with PTRACE_TRACEME to enable debugging on a process. You should use
/// pre_exec with Command to call this in the child process.
fn child_traceme() -> Result<(), std::io::Error> {
    ptrace::traceme().or(Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "ptrace TRACEME failed",
    )))
}

pub struct Inferior {
    child: Child,
    /// Maps breakpoint address -> breakpoint metadata (original byte, etc.).
    breakpoints: HashMap<usize, Breakpoint>,
}

impl Inferior {
    /// Attempts to start a new inferior process. Returns Some(Inferior) if successful, or None if
    /// an error is encountered.
    pub fn new(target: &str, args: &Vec<String>, breakpoints: &[usize]) -> Option<Inferior> {
        let mut command = Command::new(target);
        command.args(args);
        // pre_exec runs in the child after fork and before exec; it is unsafe in
        // the stdlib because the closure must be async-signal-safe.
        unsafe {
            command.pre_exec(child_traceme);
        }
        let child = command.spawn().ok()?;
        let mut inferior = Inferior {
            child,
            breakpoints: HashMap::new(),
        };
        // After TRACEME + exec, the child stops with SIGTRAP. Wait for that stop
        // so continue_exec can resume it from a known paused state.
        inferior.wait(None).ok()?;
        for &addr in breakpoints {
            if let Err(err) = inferior.set_breakpoint(addr) {
                println!("Error setting breakpoint at {:#x}: {}", addr, err);
            }
        }
        Some(inferior)
    }

    fn write_byte(&mut self, addr: usize, val: u8) -> Result<u8, nix::Error> {
        let aligned_addr = align_addr_to_word(addr);
        let byte_offset = addr - aligned_addr;
        let word = ptrace::read(self.pid(), aligned_addr as ptrace::AddressType)? as u64;
        let orig_byte = ((word >> (8 * byte_offset)) & 0xff) as u8;
        let masked_word = word & !(0xff << (8 * byte_offset));
        let updated_word = masked_word | ((val as u64) << (8 * byte_offset));
        ptrace::write(
            self.pid(),
            aligned_addr as ptrace::AddressType,
            updated_word as *mut std::ffi::c_void,
        )?;
        Ok(orig_byte)
    }

    pub fn set_breakpoint(&mut self, addr: usize) -> Result<(), nix::Error> {
        if self.breakpoints.contains_key(&addr) {
            return Ok(());
        }
        let orig_byte = self.write_byte(addr, 0xcc)?;
        self.breakpoints.insert(addr, Breakpoint { addr, orig_byte });
        Ok(())
    }

    /// Continues execution of a stopped inferior. Returns once the inferior stops
    /// or exits again.
    pub fn continue_exec(&mut self) -> Result<Status, nix::Error> {
        let regs = ptrace::getregs(self.pid())?;
        let rip = regs.rip as usize;

        // After hitting int3, %rip is one byte past the breakpoint address.
        if let Some(breakpoint) = self.breakpoints.get(&(rip.wrapping_sub(1))).cloned() {
            return self.continue_exec_with_breakpoint(breakpoint);
        }

        ptrace::cont(self.pid(), None)?;
        self.wait(None)
    }

    /// Step over a hit breakpoint, then continue until the next stop/exit.
    ///
    /// When stopped at a breakpoint, %rip points to addr+1 (after int3). We must:
    /// 1. restore the original instruction byte
    /// 2. rewind %rip back to the breakpoint
    /// 3. single-step that one real instruction
    /// 4. re-plant 0xcc
    /// 5. continue
    fn continue_exec_with_breakpoint(
        &mut self,
        breakpoint: Breakpoint,
    ) -> Result<Status, nix::Error> {
        // 1. Put the original instruction back
        self.write_byte(breakpoint.addr, breakpoint.orig_byte)?;

        // 2. Rewind %rip to the breakpoint address
        let mut regs = ptrace::getregs(self.pid())?;
        regs.rip = breakpoint.addr as u64;
        ptrace::setregs(self.pid(), regs)?;

        // 3. Execute just that instruction
        ptrace::step(self.pid(), None)?;
        match self.wait(None)? {
            Status::Exited(status) => return Ok(Status::Exited(status)),
            Status::Signaled(signal) => return Ok(Status::Signaled(signal)),
            Status::Stopped(_, _) => {
                // 4. Re-install the breakpoint so it can fire again later
                self.write_byte(breakpoint.addr, 0xcc)?;
            }
        }

        // 5. Resume until the next stop or exit
        ptrace::cont(self.pid(), None)?;
        self.wait(None)
    }

    /// Kills the inferior process.
    pub fn kill(&mut self) {
        println!("Killing running inferior (pid {})", self.pid());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Returns the pid of this inferior.
    pub fn pid(&self) -> Pid {
        nix::unistd::Pid::from_raw(self.child.id() as i32)
    }

    /// Prints a stack backtrace using %rip/%rbp and DWARF info.
    pub fn print_backtrace(&self, dwarf_data: &DwarfData) -> Result<(), nix::Error> {
        let regs = ptrace::getregs(self.pid())?;
        let mut instruction_ptr = regs.rip as usize;
        let mut base_ptr = regs.rbp as usize;
        let mut frame_num = 0;

        loop {
            let function = dwarf_data
                .get_function_from_addr(instruction_ptr)
                .unwrap_or_else(|| "<unknown>".to_string());
            match dwarf_data.get_line_from_addr(instruction_ptr) {
                Some(line) => {
                    println!(
                        "#{} {:#x} in {} ({})",
                        frame_num, instruction_ptr, function, line
                    );
                }
                None => {
                    println!("#{} {:#x} in {}", frame_num, instruction_ptr, function);
                }
            }

            // End of stack: saved %rbp is NULL
            if base_ptr == 0 {
                break;
            }
            // Return address is at %rbp + 8; previous frame's %rbp is at %rbp
            instruction_ptr = self.read_usize(base_ptr + 8)?;
            base_ptr = self.read_usize(base_ptr)?;
            frame_num += 1;
        }
        Ok(())
    }

    fn read_usize(&self, addr: usize) -> Result<usize, nix::Error> {
        let word = ptrace::read(self.pid(), addr as ptrace::AddressType)?;
        Ok(word as usize)
    }

    /// Calls waitpid on this inferior and returns a Status to indicate the state of the process
    /// after the waitpid call.
    pub fn wait(&self, options: Option<WaitPidFlag>) -> Result<Status, nix::Error> {
        Ok(match waitpid(self.pid(), options)? {
            WaitStatus::Exited(_pid, exit_code) => Status::Exited(exit_code),
            WaitStatus::Signaled(_pid, signal, _core_dumped) => Status::Signaled(signal),
            WaitStatus::Stopped(_pid, signal) => {
                let regs = ptrace::getregs(self.pid())?;
                Status::Stopped(signal, regs.rip as usize)
            }
            other => panic!("waitpid returned unexpected status: {:?}", other),
        })
    }
}
