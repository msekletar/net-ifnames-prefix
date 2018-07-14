#!/usr/bin/env ruby

require 'test/unit'
require 'fileutils'

class DeviceEnumerationTest < Test::Unit::TestCase

  def setup
    Kernel.system 'cd ..; cargo build --release'
    create_sandbox
  end

  def setup_sandbox
    FileUtils::mkdir_p 'test-sandbox'
    FileUtils::mkdir_p 'test-sandbox/etc/systemd/network/'
    FileUtils::cp 'net0.mockdev', 'test-sandbox/net0.mockdev'
  end

  def test_device_enumeration
    
    
  end
  
end
