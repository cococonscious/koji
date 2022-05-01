#!/usr/bin/env bash

set -ex

pack() {
    local tempdir
    local out_dir
    local package_name
    local gcc_prefix
    local extension

    tempdir=$(mktemp -d 2>/dev/null || mktemp -d -t tmp)
    out_dir=$(pwd)
    package_name="$PROJECT_NAME-${GITHUB_REF/refs\/tags\//}-$TARGET"

    if [[ $TARGET == "aarch64-unknown-linux-gnu" ]]; then
        gcc_prefix="aarch64-linux-gnu-"
    else
        gcc_prefix=""
    fi

    if [[ $TARGET == *"windows"* ]]; then
        extension=".exe"
    else
        extension=""
    fi

    mkdir "$tempdir/$package_name"

    cp "target/$TARGET/release/$PROJECT_NAME$extension" "$tempdir/$package_name/"
    
    if [[ $OS_NAME != "windows-latest" ]]; then
        "${gcc_prefix}"strip "$tempdir/$package_name/$PROJECT_NAME$extension"
    fi

    cp LICENSE "$tempdir/$package_name"

    pushd "$tempdir"
    if [[ $OS_NAME == "windows-latest" ]]; then
        7z a "$out_dir/$package_name.zip" "$package_name"/*
    else
        tar czf "$out_dir/$package_name.tar.gz" "$package_name"/*
    fi
    popd

    rm -r "$tempdir"
}

pack
