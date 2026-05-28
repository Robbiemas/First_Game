@echo off
setlocal

set "MOLE_DEBUG_INPUTS=1"
set "MOLE_DEBUG_INPUT_LOG=1"

call "%~dp0Launch Mole Game.cmd" %*
