@echo off
setlocal
if not exist ..\..\..\exec (
  echo "--- Creating exec directory"
  mkdir ..\..\..\exec
)

cd ..
cargo build --release --target x86_64-pc-windows-msvc

set error_var=%errorlevel%
if %error_var%==0 (
  copy target\x86_64-pc-windows-msvc\release\th_to_csv.exe ..\..\..\exec\th_to_csv_win64_rust.exe
  echo.
  echo Build succeeded
  echo.
  endlocal
  exit /b 0
) else (
  echo.
  echo Build failed
  echo.
  endlocal
  exit /b %error_var%
)
