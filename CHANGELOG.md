# Changelog
# 2.2.6 (2025-04-xx)
- !BREAKING_CHANGE! all docker images have now m3u-filter under `/app` 
- !BREAKING CHANGE! bandwidth `throttle_kbps` attribute for `reverse_proxy.stream` in  `config.yml`
  is now `throttle` and supports units. Allowed units are `KB/s`,`MB/s`,`KiB/s`,`MiB/s`,`kbps`,`mbps`,`Mibps`.
Default unit is `kbps`.
- !BREAKING_CHANGE!  `log` config `active_clients` renamed to `log_active_user`
- !BREAKING_CHANGE! user has now the attribute `ui_enabled` to disable/enable web_ui for user.
  You need to migrate the user db if you have used `use_user_db:true`.
  Set it to `false` run old m3u-filter version, then update m3u-filter and set `use_user_db:true`and start.
- !BREAKING_CHANGE! `web_ui config` restructured and added `user_ui_enabled` attribute
```yaml
web_ui:
  enabled: true
  user_ui_enabled: true
  path:
  auth:
    enabled: true
    issuer: m3u_filter
    secret: ef9ab256a8c0abe5de92c2e05ca92baa810472ab702ff1674e9248308ceeec92
    userfile: user.txt
```
- `grace_period_millis` default set to 2000 milliseconds.
- `grace_period_timeout_secs` default set to 5 seconds.
- Fixed user grace period
- Added `default_grace_period_timeout_secs` to `reverse_proxy.stream` config. When grace_period granted,
until the `default_grace_period_timeout_secs` elapses no grace_period is granted again.
- Added `method` attribute to input config. It can be set to `GET` or `POST`.
- Added optional `auto_epg` field to `input epg config` for auto-generating provider epg link.
- Added rate limiting per IP. The burst_size defines the initial number of available connections,
  while period_millis specifies the interval at which one connection is replenished.
  If behind a proxy `x-forwarded-for`, `x-real-ip` or `forwarded` should be set as header.
  The configuration below allows up to 10 connections initially and then replenishes 1 connection every 500 milliseconds.
```yaml
reverse_proxy:
  rate_limit:
    enabled: true
    period_millis: 500
    burst_size: 10
```
- Multi epg processing/optimization, auto guessing/assigning epg id's
- Fixed hls redirect url issue
- Added `force_redirect` to target config options. valid options are `live`, `vod`, `series`
```yaml
 options: {ignore_logo: false, share_live_streams: false, force_redirect: [vod, series]}
```

```yaml
epg:
  url: ['http://localhost:3001/xmltv.php?epg_id=1', 'http://localhost:3001/xmltv.php?epg_id=2']
  smart_match:
    enabled: true
    fuzzy_matching: true
    match_threshold: 80
    best_match_threshold: 99
    name_prefix: !suffix "."
    name_prefix_separator: [':', '|', '-']
    strip :  ["3840p", "uhd", "fhd", "hd", "sd", "4k", "plus", "raw"]
    normalize_regex: '[^a-zA-Z0-9\-]'
```
`match_threshold`is optional and if not set 80.
`best_match_threshold` is optional and if not set 99. 
`name_prefix` can be `ignore`, `suffix`, `prefix`. For `suffix` and `prefix` you need to define a concat string.
`strip :  ["3840p", "uhd", "fhd", "hd", "sd", "4k", "plus", "raw"]`  this is the defualt
`normalize_regex: [^a-zA-Z0-9\-]`   is the default

```yaml
# single epg
url: 'https://localhost.com/epg.xml'
```
```yaml
# multi local file  epg
url: ['file:///${env:M3U_FILTER_HOME}/epg.xml', 'file:///${env:M3U_FILTER_HOME}/epg2.xml']
```
```yaml
# multi url  epg
url: ['http://localhost:3001/xmltv.php?epg_id=1', 'http://localhost:3001/xmltv.php?epg_id=2']
```
- Added `estrip` to input for auto epg matching, if not given `["3840p", "uhd", "fhd", "hd", "sd", "4k", "plus", "raw"]` is default
  When no matching epg_id is found, the display name is used to match a channel name. The given strings are stripped to get a better match.
- Fixed chno assignment issue
- Redirect Proxy  provider cycle implemented (m3u playlist only cycles when output param `mask_redirect_url` is set). 
- Reverse Proxy mode for user can now be a subset
  - `reverse`           -> all reverse
  - `reverse[live]`     -> only live reverse, vod and series redirect
  - `reverse[live,vod]` -> series redirect, others reverse
- `/status` api endpoint moved to  `/api/v1/status` for auth protection
- fixed multi provider VOD seek problem (provider cycle  on seek request prevented playback)
- hdhomerun supports now basic auth like <http://user:password@ip:port/lineup.json>  
you need to enable auth in config
```yaml
hdhomerun:
  enabled: true
  auth: true
  devices:
  - name: hdhr1
```
- A new filter field `caption` has been added. This field is used to bypass the `title/name` issue. 
If `caption` is provided, its value is read from `title` if available, otherwise from `name`. 
When setting `caption`, both `title` and `name` are updated.”
- Counter has now an attribute padding. Which fills the number like 001.
- Added proxy configuration for all outgoing requests in `config.yml`. supported http, https, socks5 proxies.
```yaml
proxy:
  url: socks5://192.168.1.6:8123
  username: uname  # <- optional basic auth
  password: secret # <- optional basic auth
```

# 2.2.5 (2025-03-27)
- fixed web ui playlist regexp search
- added `web_ui_path` to `config.yml`
- added grace period `grace_period_millis`  attribute for `reverse_proxy.stream` in  `config.yml`
  If you have a provider or a user where the max_connection attribute is greater than 0,
  a grace period can be given during the switchover.
  If this period is set too short, it may result in access being denied in some cases.
  The default is 1000 milliseconds (1sec).
- added bandwidth `throttle_kbps` attribute for `reverse_proxy.stream` in  `config.yml`

| Resolution      |Framerate| Bitrate (kbps) | Quality     |
|-----------------|---------|----------------|-------------|
|480p (854x480)   |  30 fps | 819–2.457      | Low-Quality |
|720p (1280x720)  |  30 fps | 2.457–5.737    | HD-Streams  |
|1080p (1920x1080)|  30 fps | 5.737–12.288   | Full-HD     |
|4K (3840x2160)   |  30 fps | 20.480–49.152  | Ultra-HD    |

# 2.2.4 (2025-03-24)
- fixed `connect_timeout_secs:0` prevents connection initiation issue.
- fixed `hdhomerun` and `strm` config check for non-existing username. 
- "Breaking CHANGE! Moved `connect_timeout_secs` is global timeout and defiend in config root and not `reverse_proxy.stream`. 

# 2.2.3 (2025-03-23)
- variable resolving for config files now for all settings
- hls reverse proxy implemented
- dash redirect implemented (reverse proxy not supported) 
- !BREAKING CHANGE! `channel_unavailable_file` is now under `custom_stream_response`,
- New custom streams `user_connections_exhausted` and `provider_connections_exhausted`added. 
```yaml
custom_stream_response:
  channel_unavailable: /home/m3u-filter/resources/channel_unavailable.ts
  user_connections_exhausted: /home/m3u-filter/resources/user_connections_exhausted.ts
  provider_connections_exhausted: /home/m3u-filter/resources/provider_connections_exhausted.ts
```
- input alias definition for same provider with same content but different credentials
```yaml
- sources:
- inputs:
  - type: xtream
    name: my_provider
    url: 'http://provider.net'
    username: xyz
    password: secret1
    aliases:
    - name: my_provider_2 
      url: 'http://provider.net'
      username: abcd
      password: secret2
  targets:
  - name: test
```
Input aliases can be defined as batches in csv files with `;` separator.
There are 2 batch input types  `xtream_batch` and `m3u_batch`.
`XtreamBatch`:

```yaml
- sources:
- inputs:
  - type: xtream_batch
    url: 'file:///home/m3u-filter/config/my_provider_batch.csv'
  targets:
  - name: test
```

```csv
#name;username;password;url;max_connections;priority
my_provider_1;user1;password1;http://my_provider_1.com:80;1;0
my_provider_2;user2;password2;http://my_provider_2.com:8080;1;0
```

`M3uBatch`:
```yaml
- sources:
- inputs:
  - type: m3u_batch
    url: 'file:///home/m3u-filter/config/my_provider_batch.csv'
  targets:
  - name: test
```

```csv
#url;max_connections;priority
http://my_provider_1.com:80/get_php?username=user1&password=password1;1;0
http://my_provider_2.com:8080/get_php?username=user2&password=password2;1;0
```
The Fields `max_connections` and `priority`are optional.
`max_connections`  will be set default to `1`. This is different from yaml config where the default is `0=unlimited`  

- added two options to reverse proxy config `forced_retry_interval_secs` and `connect_timeout_secs`
`forced_retry_interval_secs` forces every x seconds a reconnect to the provider,
`connect_timeout_secs` tries only x seconds for connection, if not successfully starts a retry. 

# 2.2.2 (2025-03-12)
- !BREAKING CHANGE! Target options moved to specific target output definitions.

target `options`:
- `ignore_logo`: `true`|`false`,
- `share_live_streams`: `true`|`false`,
- `remove_duplicates`: `true`|`false`,

target output type `xtream`:
- `skip_live_direct_source`: `true`|`false`,
- `skip_video_direct_source`: `true`|`false`,
- `skip_series_direct_source`: `true`|`false`,
- `resolve_series`: `true`|`false`,
- `resolve_series_delay`: seconds,
- `resolve_vod`: `true`|`false`,
- `resolve_vod_delay`: `true`|`false`,

target output type `m3u`:
- `filename`: _optional_
- `include_type_in_url`: `true`|`false`,
- `mask_redirect_url`: `true`|`false`,

target output type `strm`:
- `directory`: _mandatory_,
- `username`: _optional_,
- `underscore_whitespace`: `true`|`false`,
- `cleanup`: `true`|`false`,
- `kodi_style`: `true`|`false`,
- `strm_props`: _optional_,  list of strings,

target output type `hdhomerun`:
- `device`: _mandatory_,
- `username`: _mandatory_,
- `use_output`: _optional_, `m3u`|`xtream`

Example:
```yaml
targets:
  - name: xc_m3u
    output:
      - type: xtream
        skip_live_direct_source: true,
        skip_video_direct_source: true,
      - type: m3u
      - type: strm
        directory: /tmp/kodi
      - type: hdhomerun
        username: hdhruser
        device: hdhr1
        use_output: xtream 
    options: {ignore_logo: false, share_live_streams: true, remove_duplicates: false}
```

- The Web UI now includes a login feature for playlist users, allowing them to set their groups for filtering and managing their own bouquet of groups.
 The playlist user can login with his credentials and can select the desired groups for his playlist.
- Added `user_config_dir` to `config.yml`. It is the storage path for user configurations (f.e. bouquets).
- New Filter field `input` can be used along `name`, `group`, `title`, `url` and `type`. Input is a `regexp` filter. `input ~ "provider\-\d+"`
- New option `use_user_db` in `api-proxy.yml`. The Playlist Users are stored inside the config file `api-proxy.yml`. When you set this option to `true`
the user are stored in a db file. This is a better choice if you have a lot of users. If you have only a few let it default to `false`
- WebUI playlist browser with tree and gallery mode. Explore self hosted and provider playlists in browser.
- Added HdHomeRun tuner target for use with Plex/Emby/Jellyfin

# 2.2.1 (2025-02-14)
- Added more info to `/status`.
- Refactored unavailable channel replacement streaming.
- Fixed catch up saving.
- Updated readme for creation of unavailable channel video file with ffmpeg for mobiles.
- refactored stream sharing.

# 2.2.0 (2025-02-11)
- !BREAKING CHANGE!  unique `input` `name` is now mandatory, because rearranging the `source.yml` could lead to wrong results without a playlist update.
- !BREAKING_CHANGE! `log_sanitize_sensitive_info`  is now under `log` section  as `sanitize_sensitive_info`
- !BREAKING_CHANGE! uuid generation for entries changed to `input.name` + `stream_id`. Virtual id mapping changed. The new Virtual id is not a sequence anymore.
- !BREAKING_CHANGE! `api-proxy.yml`  server config changed.
```yaml
server:
- name: default
  protocol: http
  host: 192.169.1.9
  port: '8901'
  timezone: Europe/Paris
  message: Welcome to m3u-filter
- name: external
  protocol: https
  host: m3ufilter.mydomain.tv
  port: '443'
  timezone: Europe/Paris
  message: Welcome to m3u-filter
  path: m3uflt
```
- Added Active clients count (for reverse proxy mode users) which is now displayed in `/status`  and can be logged with setting
`active_clients: true` under `log`section in `config.yml`
- Fixed iptv player using live tv stream without `/live/` context.
- Added `log_level` to `log` config. Priority:  CLI-Argument, Env-Var, Config, Default(`info`)
```yaml
log:
  sanitize_sensitive_info: false
  active_clients: true
  log_level: debug
update_on_boot: false
web_ui_enabled: true
```
- Added new option to `input` `xtream_live_stream_without_extension`. Default is `false`.  Some providers don't like `.ts`  extension, some providers need it.
  Now you can disable or enable it for a provider.
- Aded new option to `input` `xtream_live_stream_use_prefix`.. Default is `true`.  Some providers don't like `/live/`  prefix for streams, some providers need it.
  Now you can disable or enable it for a provider.
- Added `path` to `api-proxy.yml` server config for simpler front reverse-proxy configuration (like nginx)  
- added `hlsr` handling.
- fixed mapper counter not incrementing.
- adding `&type=m3u_plus` at the end of an `m3u` url wil trigger a download. Without it will only stream the result. 
- `kodi` `strm` generation, does not delete root directory, avoids unchanged file creations.
  `strm` files now o get timestamp from `addedd`property if exists.
- shared live stream implementation refactored.
- added optional user properties: `max_connections`, `status`, `exp_date` (expiration date as unix seconds). 
If they exist they are checked when `config.yml` `user_access_control` set to true., if you don't need them remove this fields from `api-proxy.yml` 
Added option in `config.yml` the option `user_access_control` to activate the checks. Default is false.  
- Added option `channel_unavailable_file` in `config.yml`. If a provider stream is not available this file content is send instead.
```yaml
update_on_boot: false
web_ui_enabled: true
channel_unavailable_file: /freeze_frame.ts
```

# 2.1.3 (2025-01-26)
- Hotfix 2.1.2, forgot to update the stream api code.  

# 2.1.2 (2025-01-26)
- `Strm` output has an additional option `strm_props`. These props are written to the strm file.
You can add properties like `#KODIPROP:seekable=true|false`, `#KODIPROP:inputstream=inputstream.ffmpeg` or `"#KODIPROP:http-reconnect=true`.
- Fixed xtream affix-processed output.
- `log_sanitize_sensitive_info`  added to `config.yml`. Default is `true`.
- added `resource_rewrite_disabled` to `reverse_proxy` config to disable resource url rewrite.
- Fixed series redirect proxy mode.
- Added `pushover.net` config to messaging.
```yaml
messaging:
   pushover:
    token: _required_
    user: _required_
    url: `optional`, default is https://api.pushover.net/1/messages.json
```

# 2.1.1 (2025-01-19)
- added new path `/status` which is an alias to `healthcheck`
- added memory usage to `/status`
- fixed VLC seeking problem when reconnect stream was enabled.
- duplicate field problem for xtream series/vod info fixed.
- fixed docker/build scripts
- fixed xtream live stream redirect bug

# 2.1.0 (2025-01-17)
- Watch files are now moved inside the `target` folder. Move them manually from `watch_<target_name>_<watched_group>.bin` to `<target_name>/watch_<watched_group>.bin` 
- No error log for xtream api when content is skipped with options `xtream_skip_[live|vod|series]`
- _experimental_:  added live channel connection sharing in reverse proxy mode. To activate set `share_live_streams` in target options.
- Added `info` and `tmdb-id` caching for vod and series with options `xtream_resolve_(series|vod)`.
- The `kodi` format for movies can contain the `tmdb-id` (_optional_). To add the `tmdb-id` you can set now `kodi_style`,  `xtream_resolve_vod`, `xtream_resolve_vod_delay`, `xtream_resolve_series` and  `xtream_resolve_series_delay` to target options.
- `kodi` output can now have `username` attribute to use reverse proxy mode when combined with `xtream` output.
- Fixed webUI manual update for selected targets
- Added m3u logo url rewrite in `reverse proxy` mode or with `m3u_mask_redirect_url` option.
- BPlusTree compression changed from zlib to zstd.
- Breaking change: multi scheduler config with optional targets. 
```yaml
#   sec  min   hour   day of month   month   day of week   year
schedules:
- schedule: "0  0  8  *  *  *  *"
  targets:
  - vod_channels
- schedule: "0  0  10  *  *  *  *"
  targets:
  - series_channels
- schedule: "0  0  20  *  *  *  *"
```
- Stats have now target information
- Prevent simultaneous updates
- Added target options `remove_duplicates` to remove entries with same `url`.
- Added reverse Proxy config to `config.yml`
- `config.yml` `backup_dir` is now default `backup`. If you want to keep the old name set `backup_dir: .backup`
```yaml
reverse_proxy:
  stream:
    retry: true
    buffer:
      enabled: true
      size: 1024
    connect_timeout_secs: 5
  cache:
    size: 500MB
    enabled: true
    dir: ./cache
```

# 2.0.10 (2024-12-03)
- added Target Output Option `m3u_include_type_in_url`, default false. This adds `live`, `movie`, `series` to the url of the stream in reverse proxy mode.
- added Target Output Option `m3u_mask_redirect_url`, default false. The urls are pointed to m3u-filter in redirect mode. In stream request a redirect response is send. Usefully if you want to track calls in redirect mode.
- fixed xtream api redirect url problem.

# 2.0.9 (2024-12-01)
- Fixed api proxy server url bug

# 2.0.8 (2024-11-27)
- The configured directories `data`, `backup` and `video-download` are created when configured and do not exist.
- set "actix_web::middleware::logger" to level `error`
- masking sensitive information in log
- hls support (m3u8 url, ignores proxy type, always redirect)

# 2.0.7 (2024-11-05)
- EPG is now first downloaded to disk instead of directly into memory, then processed using a SAX parser (slower but reduces memory usage from up to 2GB).
- Various code optimizations have been applied.
- Regular expression matching in log output is now set to trace level to prevent flooding the debug log.
- Processing stats now include a `took` field indicating the processing time.

# 2.0.6 (2024-11-02)
- breaking change virtual_id handling. You need to clear the data directory.
- new content storage implementation with BPlusTree indexing.
- api responses are now streamed directly from disk to avoid memory allocation.
- fixed scheduler implementation to only wake up on scheduled times.
- 
# 2.0.5(2024-10-16)
- input url supports now scheme `file://...` (which is not necessary because file paths are supported). Gzip files are also supported.     
- sort takes now a sequence for channel values which has higher priority than sort order
- fixed error handling in filter parsing
- `NOT` filter is now `non greedy`. `NOT Name ~ "A" AND Group ~ "B"` was `NOT (Name ~ "A" AND Group ~ "B")`. Now it is `(NOT Name ~ "A") AND Group ~ "B"`  
- Implemented workaround for missing tvg-ID

# 2.0.4(2024-09-19)
* if Content type of file download is not set in header, the gzip encoding is checked through magic header.
* if source is m3u and stream id not a number, the entry is skipped and logged.
* prefix and suffix was applied wrong, fixed.
* epg timeshift, define timeshift api-proxy.yml for each user as `epg_timeshift: hh:mm`, example  `-2:30`, `1:45`, `+0:15`, `2`, `:30`, `:3`, `2:`
* timeshift.php api implementation
* New Filter `type` added can be uses as  `Type = vod` or `Type = live` or `Type = series`
* Counter in `mapping.yml`. Each mapper can have counters to add counter to specific fields.
* Added new mapper feature `transform`. `uppercase`, `lowercase` and `capitalize` supported.
* Fixed parsing invalid m3u playlist entries like `tvg-logo="[""]"`

# 2.0.3(2024-07-11)
*  added  `source` - `input` - `name` attribute to README
*  added `chno`  to Playlist attributes.
*  `epg_channel_id` mapping fixed 

# v2.0.2(2024-05-28)
* Added Encoding handling: gzip,deflate 
* Fixed panic when `tvg-id` is not set.

# v2.0.1(2024-05-24)
* m3u playlists are not saved as plainfile, therefor m3u output filename is not mandatory, if given the plain m3u playlist is stored.
* Added `--healthcheck` argument for docker 
* Added `catch-up`/`timeshift`  api for `xtream`

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
schedules:
  - schedule: '0  0  0,8,18  *  *  *  *'
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
