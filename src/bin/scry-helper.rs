use std::process::Command;
use std::os::unix::process::CommandExt;
use std::ffi::CString;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let app_source = &args[1];
    let use_uts_namespace = args[2] == "true";
    let command = &args[3];
    let command_args = &args[4..];


    if use_uts_namespace {
        let hostname = CString::new("scry-sandbox")
            .expect("Invalid hostname");

        let hostname_result = unsafe {
            libc::sethostname(
                hostname.as_ptr(),
                "scry-sandbox".len(),
            )
        };

        if hostname_result != 0 {
            panic!(
                "Could not set sandbox hostname: {}",
                std::io::Error::last_os_error()
            );
        }
    }


    std::fs::create_dir_all("/tmp/scry-root/dev")
        .expect("Could not create /dev");

    std::fs::File::create("/tmp/scry-root/dev/null")
        .expect("Could not create /dev/null mount point");

    let dev_null_source = CString::new("/dev/null")
        .expect("Invalid /dev/null path");

    let dev_null_target = CString::new("/tmp/scry-root/dev/null")
        .expect("Invalid sandbox /dev/null path");

    let dev_null_mount_result = unsafe {
        libc::mount(
            dev_null_source.as_ptr(),
            dev_null_target.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };

    if dev_null_mount_result != 0 {
        panic!(
            "Could not bind-mount /dev/null: {}",
            std::io::Error::last_os_error()
        );
    }


    std::fs::create_dir_all("/tmp/scry-root")
        .expect("Could not create sandbox root");

    let root = CString::new("/tmp/scry-root")
        .expect("Invalid sandbox root path");

    let mount_result = unsafe {
        libc::mount(
            root.as_ptr(),
            root.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };

    if mount_result != 0 {
        panic!("Could not bind-mount sandbox root: {}", std::io::Error::last_os_error());
    }



    std::fs::create_dir_all("/tmp/scry-root/app")
        .expect("Could not create /app");

    let app_source_c = CString::new(app_source.as_str())
        .expect("Invalid app source path");

    let app_target = CString::new("/tmp/scry-root/app")
        .expect("Invalid sandbox /app path");

    let app_mount_result = unsafe {
        libc::mount(
            app_source_c.as_ptr(),
            app_target.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };

    if app_mount_result != 0 {
        panic!(
            "Could not bind-mount app directory: {}",
            std::io::Error::last_os_error()
        );
    }


    std::fs::create_dir_all("/tmp/scry-root/tmp")
        .expect("Could not create /tmp");

    std::fs::create_dir_all("/tmp/scry-root/usr")
        .expect("Could not create /usr");

    std::fs::create_dir_all("/tmp/scry-root/lib64")
        .expect("Could not create /lib64");



    let usr_source = CString::new("/usr")
        .expect("Invalid /usr path");

    let usr_target = CString::new("/tmp/scry-root/usr")
        .expect("Invalid sandbox /usr path");

    let usr_mount_result = unsafe {
        libc::mount(
            usr_source.as_ptr(),
            usr_target.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };

    if usr_mount_result != 0 {
        panic!(
            "Could not bind-mount /usr: {}",
            std::io::Error::last_os_error()
        );
    }



    let lib64_source = CString::new("/lib64")
        .expect("Invalid /lib64 path");

    let lib64_target = CString::new("/tmp/scry-root/lib64")
        .expect("Invalid sandbox /lib64 path");

    let lib64_mount_result = unsafe {
        libc::mount(
            lib64_source.as_ptr(),
            lib64_target.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };

    if lib64_mount_result != 0 {
        panic!(
            "Could not bind-mount /lib64: {}",
            std::io::Error::last_os_error()
        );
    }



    std::env::set_current_dir("/tmp/scry-root")
        .expect("Could not enter sandbox root");

    let dot = CString::new(".")
        .expect("Invalid dot path");

    let pivot_result = unsafe {
        libc::syscall(
            libc::SYS_pivot_root,
            dot.as_ptr(),
            dot.as_ptr(),
        )
    };

    if pivot_result != 0 {
        panic!(
            "Could not pivot root: {}",
            std::io::Error::last_os_error()
        );
    }

    let detach_result = unsafe {
        libc::umount2(
            dot.as_ptr(),
            libc::MNT_DETACH,
        )
    };

    if detach_result != 0 {
        panic!(
            "Could not detach old root: {}",
            std::io::Error::last_os_error()
        );
    }

    std::env::set_current_dir("/app")
        .expect("Could not enter /app");



    let error = Command::new(command)
        .args(command_args)
        .exec();

    eprintln!("scry-helper: failed to exec target: {error}");
    std::process::exit(127);
}