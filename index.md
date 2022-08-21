# Introduction

Cpplumber is a static analysis tool that helps detecting and keeping track of
C and C++ source code information that leaks into compiled executable files.

This tool is aimed at people developing software that may contain sensitive
information in some debug or private configurations and want to make sure it
doesn't go out accidentally in release builds or for people that just want to
make sure reverse engineers don't have it too easy on their software. 

