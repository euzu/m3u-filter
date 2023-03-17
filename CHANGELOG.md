# Changelog
# v1.0(2023-03-17)
* Fixed template dependency replacement.
* Added optional 'name' property to target. Default is 'default'.
* Added Dockerfile

# Changelog
# v0.9.9(2023-03-03)
* Added optional 'enabled' property to input and target. Default is true.  

# Changelog
# v0.9.8(2023-02-25)
* Added new fields to mapping attributes and assignments
  - "name"
  - "title"
  - "group"
  - "id"
  - "chno"
  - "logo"
  - "logo_small"
  - "parent_code"
  - "audio_track"
  - "time_shift"
  - "rec"
  - "source"
* Added static suffix and prefix at inpupt source level 

# v0.9.7(2023-02-15)
* Breaking changes, mappings.yml refactored 
* Added `threads` property to config, which executes different sources in threads.
* WebUI: Added clipboard collector on left side 
* Added templates to config to use in filters
* Added nested templates, templates can have references to other templates with `!name!`. 
* Renamed Enum Constants
  - M3u -> m3u,
  - Strm -> strm 
  - FRM -> frm 
  - FMR -> fmr 
  - RFM -> rfm 
  - RMF -> rmf 
  - MFR -> mfr 
  - MRF -> mrf 
  - Group -> group   (Not in filter regular expressions)
  - Name -> name  (Not in filter regular expressions)
  - Title -> title  (Not in filter regular expressions)
  - Url -> url  (Not in filter regular expressions)
  - Discard -> discard 
  - Include -> include 
  - Asc -> asc 
  - Desc -> desc 

# v0.9.6(2023-01-14)
* Renamed `mappings.templates` attribute `key` to `name`
* `mappings.tag` is now a struct
  - captures: List of captured variable names like `quality`.
  - concat: if you have more than one captures defined this is the join string between them
  - suffix: suffix for thge tag
  - prefix: prefix for the tag

# v0.9.5(2023-01-13)
* Upgraded libraries, fixed serde_yaml v.0.8 empty string bug.
* Added Processing Pipe to target for filter, map and rename. Values are: 
  - FRM
  - FMR 
  - RFM 
  - RMF 
  - MFR
  - MRF
default is FMR
* Added mapping parameter `match_as_ascii`. Default is `false`. 
If `true` before regexp matching the matching text will be converted to ascii. [unidecode](https://chowdhurya.github.io/rust-unidecode/unidecode/index.html)

Added regexp templates to mapper:
```yaml
mappings:
  - id: France
    tag: ""
    match_as_ascii: true
    templates:
      - key: delimiter
        value: '[\s_-]*'
      - key: quality
        value: '(?i)(?P<quality>HD|LQ|4K|UHD)?'
    mapper:
      - tvg_name: TF1 $quality
        # https://regex101.com/r/UV233E/1
        tvg_names:
          - '^\s*(FR)?[: |]?TF1!delimiter!!quality!\s*$'
        tvg_id: TF1.fr
        tvg_chno: "1"
        tvg_logo: https://emojipedia-us.s3.amazonaws.com/source/skype/289/shrimp_1f990.png
        group_title:
          - FR
          - TNT
```

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
