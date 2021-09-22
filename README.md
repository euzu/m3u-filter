#m3u-filter

m3u-filter is a simple application which can filter entries out of a playlist in EXTM3U format.
If you have a playlist which contains unwanted entries, you can create filter which Include or Discard entries
based on the header information of the playlist entries.
Currently filter and rename operations support group, name and title fields.

You can define multiple targets for filtering if you want to create multiple playlists from a big playlist.

The config.yml file contains the filter and rename definitions. It should be located next to the exe file or in the current working directory.
You can override this behaviour with the -c argument.
The input file can be defined inside the config.yml file or can be given as -i argument.
If given as argument, it overrides the config file entry.

the config.yml looks like:
```yaml
***REMOVED***
  filename: playlist.m3u
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
***REMOVED***
```

The filter is either Include mode or Discard mode.
The regular expression syntax is similar to Perl-style regular expressions,
but lacks a few features like look around and backreferences.

The rename supports capture groups. Each group can be adressed with $1, $2 .. in the new_name attribute.
This is needed for players which do not observe the order and sort themselves. In the above example each entry starting
with DE will be prefixed with "1.". 
If you dont care about sorting, you dont need the rename block.


##The EXTM3U format is an extension of the M3U format.
m3u has become almost a standard for the formation of playlists of media players and media devices.

A file in the EXTM3U format is a text file with the extension m3u or m3u8.

An example of the contents of the file in the EXTM3U format
```
#EXTM3U
#EXTINF:-1 tvg-name="Channel 1" tvg-logo="http://site.domain / channel1_logo.png" group-title="Group 1",Channel 1
http://site.domain/channel1
#EXTINF:-1 tvg-name="Channel 2" tvg-logo="http://site.domain / channel2_logo.png"  group-title="Group 2",Channel 2
http://site.domain/channel2
#EXTINF:-1 tvg-name="Channel 3" tvg-logo="http://site.domain / channel3_logo.png"  group-title="Group 2",Channel 3
http://site.domain/channel3 -- reports and targets
```
