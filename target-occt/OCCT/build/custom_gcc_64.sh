#!/bin/bash

if [ "$1" == "" ]; then
  if [ "$2" == "64" ]; then
    # set environment variables used by OCCT
    export CSF_FPE=0

    export TCL_DIR=""
    export TK_DIR=""
    export FREETYPE_DIR=""
    export FREEIMAGE_DIR=""
    export TBB_DIR=""
    export VTK_DIR=""
    export FFMPEG_DIR=""
    export JEMALLOC_DIR=""

    if [ "x@3RDPARTY_QT_DIR" != "x" ]; then
      export QTDIR=""
    fi

    export TCL_VERSION_WITH_DOT=""
    export TK_VERSION_WITH_DOT=""

    export CSF_OCCTBinPath="/home/user/forma/target-occt/OCCT/build/lin64/gcc/bin"
    export CSF_OCCTLibPath="/home/user/forma/target-occt/OCCT/build/lin64/gcc/lib"
    export CSF_OCCTIncludePath="/home/user/forma/target-occt/OCCT/build/inc"
    export CSF_OCCTResourcePath="/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-7.8.1/OCCT/src"
    export CSF_OCCTDataPath="/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-7.8.1/OCCT/data"
    export CSF_OCCTSamplesPath="/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-7.8.1/OCCT/samples"
    export CSF_OCCTTestsPath="/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-7.8.1/OCCT/tests"
    export CSF_OCCTDocPath="/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-7.8.1/OCCT/doc"

    # for compatibility with external application using CASROOT
    export CASROOT="/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/occt-sys-7.8.1/OCCT"
  fi
fi

