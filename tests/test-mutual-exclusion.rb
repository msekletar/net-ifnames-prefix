#!/usr/bin/env ruby

require 'io/console'
require 'test/unit'

$NUMBER_OF_THREADS = 1000

class MutualExclusionTest < Test::Unit::TestCase

  def setup
    Kernel.system 'cd ..; cargo build --release'
  end

  def teardown
    File.delete('test.log')
  end

  def check_output
    lock = /lock taken by PID=\d+/
    unlock = /lock released by PID=\d+/
    ifname = /ID_NET_IFNAME_PREFIX=/

    File.open('test.log') do |f|
      while (l = f.gets)
        n = f.gets
        u = f.gets

        assert_not_nil lock.match(l)
        assert_not_nil ifname.match(n)
        assert_not_nil unlock.match(u)
      end
    end
  end

  def test_concurrent_run
    threads = []
    start = 0

    # create threads
    (1..$NUMBER_OF_THREADS).each do
      threads << Thread.new do
        loop do
          break if start != 0
          sleep 0.01
        end
        Kernel.system '../target/release/net-ifnames-prefix 2>&1 >> test.log'
      end
    end

    start = 1
    threads.each(&:join)

    check_output
  end
end
