#!/bin/bash

TARGET_DIR=$1
TODAY=$(date +%y%m%d)

if [ ! -d $TARGET_DIR ]
then
	echo "$TARGET_DIR is not exists or not a direcotry"
	exit 1
fi

OLD_CURRENT=$PWD

cd "$TARGET_DIR" || exit 1

PREFIXES=$(find . -name '*.log.analyze' \
	| sed -E 's#.*/(.*_[0-9]+)_[0-9]+.log.analyze#\1#' \
	| sort \
	| uniq)

for p in $PREFIXES
do
	if echo "$p" | grep -q "$TODAY"
	then
		continue
	fi
	# shellcheck disable=SC2086
	tar cfz "$p.tar.gz" $p*.log.analyze
	# shellcheck disable=SC2086
	rm  $p*.log.analyze
done

cd "$OLD_CURRENT" || exit