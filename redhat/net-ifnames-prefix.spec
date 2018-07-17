Name:           net-ifnames-prefix
Version:        0.1.0
Release:        1%{?dist}
Summary:        Udev helper utility that provides network interface naming using user defined prefix

License:        MIT
URL:            https://www.github.com/msekletar/net-ifnames-prefix
Source0:        https://www.github.com/msekletar/net-ifnames-prefix/archive/%{name}-%{version}.tar.xz

ExclusiveArch: %{rust_arches}

BuildRequires:  rust-packaging
# [dependencies]
BuildRequires: (crate(libudev) >= 0.2 with crate(libudev) < 0.3)
BuildRequires: (crate(regex) >= 1.0.1 with crate(regex) < 2.0.0)
BuildRequires: (crate(hwaddr) >= 0.1.4 with crate(hwaddr) < 0.2.0)
BuildRequires: (crate(rust-ini) >= 0.12.2 with crate(rust-ini) < 0.13.0)
BuildRequires: (crate(libc) >= 0.2.40 with crate(libc) < 0.3.0)
BuildRequires: (crate(log) >= 0.4.1 with crate(log) < 0.5.0)
BuildRequires: (crate(env_logger) >= 0.5.10 with crate(env_logger) < 0.6.0)

%description
This package provides udev helper utility that tries to consistently name all ethernet NICs using
user defined prefix (e.g. net.ifnames.prefix=net produces NIC names net0, net1, ...). Utility is
called from udev rule and it determines NIC name and writes out configuration file for udev's
net_setup_link built-in (e.g. /etc/systemd/network/70-net-ifnames-prefix-net0.link).

%prep
%autosetup -S git_am
%cargo_prep

%build
%cargo_build

%install
%cargo_install

%files
%defattr(-,root,root,-)
%license LICENSE
%doc README.md


%changelog
* Sun Jul 15 2018 Michal Sekletar <msekleta@redhat.com>
- initial package
