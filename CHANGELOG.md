# Changelog
# v0.9.5(2023-01-13)
* Upgraded libraries, fixed serde_yaml v.0.8 empty string bug.
* Added Processing Pipe to target for filter, map and rename. Values are: 
  - Frm
  - Fmr 
  - Rfm 
  - Rmf 
  - Mfr
  - Mrf
default is Fmr
* Added mapping parameter `match_as_ascii`. Default is `false`. 
If `true` before regexp matching the matching text will be converted to ascii. [unidecode](https://chowdhurya.github.io/rust-unidecode/unidecode/index.html)

* `mapping` attribute for target is now a list. You can assign multiple mapper to a target.
```
mapping:
  - France
  - Belgium
  - Germany
```

# v0.9.4(2023-01-12)
* Added mappings. Mappings are defined in a file named ```mapping.yml``` or can be given by command line option ```-m```.
```target``` has now an optional field ```mapping``` which has the id of the mapping configuration.   
* rename is now optional

# v0.9.3(2022-04-21)
* ```Strm``` output has an additional option ```kodi_style```. This option tries to guess the year, season and episode for kodi style names.
https://kodi.wiki/view/Naming_video_files/TV_shows

# v0.9.2(2022-04-05)
* ```Strm``` output has an additional option ```cleanup```. This deletes the old directory given at ```filename```.

# v0.9.1(2022-04-05)
* There are two types of targets ```m3u``` and ```strm```. This can be set by the ```output``` attribute to ```Strm``` or ```M3u```.
If the attribute is not specified ```M3u``` is created by default. ```Strm``` output has an additional option ```underscore_whitespace```. This replaces all whitespaces with ```_``` in the path.

## v0.9.0(2022-04-04)
* Changed filter. Filter are now defined as filter statements. Url added to filter fields.

## v0.8.0(2022-03-24)
* Changed configuration. It is now possible to handle multiple sources. Each input has its own targets.

## v0.7.0(2022-01-20)
* Updated frontend libraries
* Added Search, currently only plain text search

## v0.6.0(2021-12-29)
* Added options to target, currently only ignore_logo
* Added sorting to groups

## v0.5.0(2021-10-15)
* Fixed: config input persistence filename was ignored 
* Added working_dir to configuration
* relative web_root is now checked for existence in current path and working_dir. 

## v0.4.0(2021-10-08)
* Fixed server exit on playlist not found
* Added copy link to clipboard in playlist tree

## v0.3.0(2021-10-08)
* Updated frontend packages
* Added linter for code checking
* Updated tree layout and added hover coloring
* Fixed Url Field could not be edited after drop down selection
* Added download on key-"Enter" press

## v0.2.0(2021-10-07)
* Added simple WEB-UI
  * Start in server mode

## v0.1.0(2021-10-01)
* Initial project release
