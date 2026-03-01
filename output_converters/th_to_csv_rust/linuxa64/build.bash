#!/bin/bash

#
# check if exec directory exists, create if not
#
if [ ! -d ../../../exec ]
then
   mkdir ../../../exec
fi

cd ..
cargo build --release --target aarch64-unknown-linux-gnu
export BUILD_RETURN_CODE=$?
if [ $BUILD_RETURN_CODE -ne 0 ]
then
   echo " "
   echo "Build failed"
   echo " "
   exit $BUILD_RETURN_CODE
fi

cp target/aarch64-unknown-linux-gnu/release/th_to_csv ../../../exec/th_to_csv_linuxa64_rust

echo " "
echo "Build succeeded"
echo " "
exit 0
