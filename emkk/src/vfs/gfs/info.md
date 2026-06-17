# General File System

the General File System (alias GFS) is a intermidate file system between the physical file systems (alias PFS)
and the Virtual File System(alias VFS). The main job of the GFS is to standardize the accessing process of a fs related operation.

## Why standardize?
The ultimate goal of this operating system is to support both linux and windows applications.
A part of this ultimate support is the file system.
Files from a Linux partition should be available for a Windows process and files from a Windows partition should be available to Linux processes.
The GFS abstracts the physical structure of a filesystem into common structures, for example GfsFsType,GfsFs, GfsNtfsRoot.
Those structures will then be accessed by a read or write function of the VFS/GFS FS layer.
When physical read or writes have to be executed, the appropiate GFS function will call a minimalistic designated function, which is build for the physical file system.
Physical accesses will be minimalized for, since directories are loaded into memory according to a certain criteria

## Current Status
  The Current status of the GFS is uncompleted, error prone on other settings (less than 4096 block size in ext2) and minimalistic, it currently only supports ext2

## Is this relly necessary?
  Kind of yes, since the ultimate goal of this operating system is kind of unrealistic and will probably take a lot of time. But hey, I´m currently 15 I got time :)

## Memory Usage
  High Memory Usage, 139 4k Pages => 569344 bytes
  For 131072 directory entry characters
