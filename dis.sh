#!/bin/bash
llvm-objdump --disassembler-color=on -d $1 | less -R
