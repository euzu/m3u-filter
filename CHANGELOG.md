# Changelog
# v2.0.1(2024-05-xx)
* m3u playlists are not saved as plainfile, therefor m3u output filename is not mandatory, if given the plain m3u playlist is stored.
* Added `--healthcheck` argument for docker 

# v2.0.0(2024-05-10)
* major version change due to massive changes
* `update_on_boot` for config, default is false, if true an update is run on start
* `category_id` filter added to xtream api
* Handling for m3u files without id and group information
* Added `panel_api.php`  endpoint for xtream
* Case insensitive filter syntax
* Xtream category_id fixes, to avoid category_id change when title not changes.
* Target options `xtream_skip_live_direct_source` and `xtream_skip_video_direct_source` are now default true
* added new target option
  - `xtream_skip_series_direct_source` default is true
* Added new options to input configuration. `xtream_skip_live`, `xtream_skip_vod`, `xtream_skip_series`
* Updated docker files, New Dockerfile with builder to build an image without installing rust or node environments.
* Generating xtream stream urls from m3u input.
* Reverse proxy implementation for m3u playlist.
* Mapper can now set `epg_channel_id`.
* Added environment variables for User Credentials `username`, `password` and `token` in format `${env:<EnvVarName>}` where `<EnvVarName>` should be replaced.
* Added `web_ui_enabled` to `config.yml`. Default is `true`. Set to `false` to disable webui.
* Added `web_auth` to `config.yml` struct for web-ui-authentication is optional.
   - `enabled`: default true
   - `issuer` issuer for jwt token
   - `secret` secret for jwt token
   - `userfile` optional userfile with generated userfile in format "username: password" per file, default name is user.txt in config path
* Password generation argument --genpwd  to generate passwords for userfile. 
* Added env var `M3U_FILTER_LOG` for log level
* Log Level has now module support like `m3u_filter::util=error,m3u_filter::filter=debug,m3u_filter=debug`
* Multiple Xtream Sources merging into one target is now supported

# v1.1.8(2024-03-06)
* Fixed WebUI Option-Select  
* WebUI: added gallery view as second view for playlist
* Breaking change config path. The config path is now default ./config. 
  You can provide a config path with the "-p" argument.

# v1.1.7(2024-01-30)
* Renamed api-proxy.yml server info field `ip` to `host`
* Multiple server-config for xtream api. In api-proxy.yml assign server config to user

# v1.1.6(2024-01-17)
* Watch filter are now regular expressions 
* Fixed watch file not created problem
* UI responds immediately to update request

# v1.1.5(2024-01-11)
* Changed api-proxy user default proxy type from `reverse` to `redirect`
* Added `xtream_resolve_series` and `xtream_resolve_series_delay` option for `m3u` target 
* Messaging calling rest endpoint added
* Messaging added 'Watch' option as OptIn

# v1.1.4(2023-12-06)
* Breaking change, `config.yml` split into `config.yml` and `source.yml`
* Added `backup_dir` property to `config.yml` to store backups of changed config files.
* Added regexp search in Web-UI
* Added config Web-UI
* Added xtream vod_info and series_info, stream seek. 
* Added input options with attribute xtream_info_cache to cache get_vod_info and get_series_info on disc
* for xtream api added proxy types reverse and redirect to user credentials.

# v1.1.3(2023-11-08)
* added new target options 
  - `xtream_skip_live_direct_source`
  - `xtream_skip_video_direct_source`
* internal optimization/refactoring to avoid string cloning.
* new options for downloading media files from web-ui 
  - `organize_into_directories`
  - `episode_pattern`
* Web-UI - Download View with multi download support
* Added WebSearch Url `web_search: 'https://www.imdb.com/search/title/?title={}'` under video configuration.

# v1.1.2(2023-11-03)
* Fixed epg for xtream
* Fixed some Web-UI Problems
* Added some convenience endpoints to rest api

# v1.1.1(2023-10-31)
* Added scheduler to update lists in server mode.
* Added Xtream Cluster Live, Video, Series. M3u Playlist cluster guessing through video file endings.
* Added api-proxy config for xtream proxy, to define server info and user credentials
* Added Xtream Api Endpoints.
* Added M3u Api Endpoints.
* Added multiple input support
* Added Messaging with opt in message types [info, error, stats]
* Added Telegram message support
* Added Target watch for groups
* Fixed TLS problem with docker scratch
* Added simple stats
* Target Output is now a list of multiple output formats, !breaking change!
* RegExp captures can now be used in mapper attributes
* Added file download to a defined directory in config
* Refactored web-ui
* Added XMLTV support

Changes in `config.yml`
```yaml
messaging:
  notify_on:
    - error
    - info
    - stats
  telegram:
    bot_token: '<your telegram bot token>'
    chat_ids:
      - <your telegram chat_id>
schedule: '0  0  0,8,18  *  *  *  *'
```

`api-proxy.yml`
```yaml
server:
  protocol: http
  ip: 192.168.9.3
  http_port: 80
  https_port:
  rtmp_port:
  timezone: Europe/Paris
  message: Welcome to m3u-filter
user:
  - target: pl1
    credentials:
      - {username: x3452, password: ztrhgrGZrt83hjerter}
```

# v1.0.1(2023-09-07)
* Refactored sorting. Sorting channels inside group now possible

# Changelog
# v1.0.0(2023-04-27)
* Added target argument for command line. `m3u-filter -t <target_name> -t <target_name>`. Target names should be provided in the config.
* Added filter to mapper definition.
* Refactored filter parsing.
* Fixed sort after mapping group names.
* Refactored mapping, fixed reading unmodified initial values in mapping loop from ValueProvider, because of cloned channel

# v0.9.9(2023-03-20)
* Added optional 'enabled' property to input and target. Default is true.  
* Fixed template dependency replacement.
* Added optional 'name' property to target. Default is 'default'.
* Added Dockerfile
* Added xtream support
* Breaking changes: config changes for input

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
