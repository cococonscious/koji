#!/usr/bin/env bash
set -ex

local tempdir
local out_dir
local package_name
local gcc_prefix

tempdir=$(mktemp -d 2>/dev/null || mktemp -d -t tmp)
out_dir=$(pwd)
package_name="$PROJECT_NAME-${GITHUB_REF/refs\/tags\//}-$TARGET"

if [[ $TARGET == "arm-unknown-linux-gnueabihf" ]]; then
    gcc_prefix="arm-linux-gnueabihf-"
elif [[ $TARGET == "aarch64-unknown-linux-gnu" ]]; then
    gcc_prefix="aarch64-linux-gnu-"
else
    gcc_prefix=""
fi

# create a "staging" directory
mkdir "$tempdir/$package_name"

# copying the main binary
cp "target/$TARGET/release/$PROJECT_NAME" "$tempdir/$package_name/"
if [ "$OS_NAME" != windows-latest ]; then
    "${gcc_prefix}"strip "$tempdir/$package_name/$PROJECT_NAME"
fi

# copy license
cp LICENSE "$tempdir/$package_name"

# archiving
pushd "$tempdir"
if [ "$OS_NAME" = windows-latest ]; then
    7z a "$out_dir/$package_name.zip" "$package_name"/*
else
    tar czf "$out_dir/$package_name.tar.gz" "$package_name"/*
fi
popd
rm -r "$tempdir"
