#!/bin/bash

#
# check if exec directory exists, create if not
#
if [ ! -d ../../../exec ]
then
   mkdir ../../../exec
fi

cd ..
cargo build --release
export BUILD_RETURN_CODE=$?
if [ $BUILD_RETURN_CODE -ne 0 ]
then
   echo " "
   echo "Build failed"
   echo " "
   exit $BUILD_RETURN_CODE
fi

cp target/release/th_to_csv ../../../exec/th_to_csv_linux64_rust

echo " "
echo "Build succeeded"
echo " "
exit 0
