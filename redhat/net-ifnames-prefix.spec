Name:           net-ifnames-prefix
Version:        0.1.0
Release:        1%{?dist}
Summary:        Udev helper utility that provides network interface naming using user defined prefix

License:        MIT
URL:            https://www.github.com/msekletar/net-ifnames-prefix
Source0:        https://www.github.com/msekletar/net-ifnames-prefix/archive/%{name}-%{version}.tar.xz

BuildRequires:  rust-packaging
ExclusiveArch: %{rust_arches}

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
