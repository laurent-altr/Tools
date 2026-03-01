@echo off
cd ..
cargo build --release --target x86_64-pc-windows-msvc
copy target\x86_64-pc-windows-msvc\release\th_to_csv.exe ..\..\..\exec\th_to_csv_win64.exe
