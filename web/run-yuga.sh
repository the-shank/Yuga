#!/usr/bin/env bash

if [ -d /tmp/mydir ]; then
	rm -rf /tmp/mydir
fi
git clone $1 /tmp/mydir 2>&1
cd /tmp/mydir
if [ "$3" != "HEAD" ]; then
	echo "Resetting HEAD to $3"
	git reset --hard $3
fi
if [ "$4" != "." ]; then
	cd "$4"
	case $PWD/ in
  		/tmp/mydir/*) echo "Changed directory to $(pwd)";;
  		*) echo "Invalid sub-path '$4'"; cd -;;
	esac
fi
cargo rudra --all-features 2>&1
if [ -d "$(pwd)/yuga_reports" ]; then
	mv "$(pwd)/yuga_reports" $2
fi
rm -rf /tmp/mydir
