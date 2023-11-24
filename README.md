# m3u-filter

m3u-filter is a simple application which can:
  - filter, rename, map and sort entries out of a playlist and persist in EXTM3U, XTREAM or Kodi format.
  - can process multiple inputs and can create multiple outputs from this input files trough target definitions.
  - act as simple xtream or m3u server after processing entries
  - can schedule updates in server mode
  - can run as cli-command for serving processed playlists through web-server like nginx or apache.
  - can define multiple targets for filtering if you want to create multiple playlists from a big playlist.
  - use regular expressions for matching
  - define filter as statements like `filter: (Group ~ "^FR.*") AND NOT(Group ~ ".*XXX.*" OR Group ~ ".*SERIES.*" OR Group ~".*MOVIES.*")`
  - DRY - define templates and use them, don't repeat yourself
  - Send a telegram bot message when something goes wrong
  - Watch changes in groups and get a message on changes

![m3u-filter-overview](https://github.com/euzu/m3u-filter/assets/33094714/9a3449ac-c646-4bb4-a5ab-320a588d35c8)

If you have a playlist which contains unwanted entries, you can create filter which include or discard entries
based on the header information of the playlist entries, you can rename entries or map entries based on regular expressions.

You can run m3u-filter as command line application to update your playlists (manually or as cron job), or you can 
run it in server mode and open the web-ui to see the contents of the playlist, filter/search content and save 
the filtered groups as a new playlist.

## Starting in server mode for Web-UI
The Web-UI is available in server mode. You need to start `m3u-filter` with the `-s` (`--server`) option.
On the first page you can select one of the defined input sources in the configuration, or write an url to the text field.
The contents of the playlist are displayed in the tree-view. Each link has one or more buttons. 
The first is for copying the url into clipboard. The others are visible if you have configured the `video`section. 
Based on the stream type, you will be able to download or search in a configured movie database for this entry.   

In the tree-view each entry has a checkbox in front. Selecting the checkbox means **discarding** this entry from the 
manual download when you hit the `Save` button.

## 1. `config.yml`

For running in cli mode, you need to define a `config.yml` file which can be next to the executable or provided with the
`-c` cli argument. It contains the filter, rename and mapping definitions.

For running specific targets use the `-t` argument like `m3u-filter -t <target_name> -t <other_target_name>`.
Target names should be provided in the config. The -t option overrides `enabled` attributes of `input` and `target` elements.
This means, even disabled inputs and targets are processed when the given target name as cli argument matches a target.

Top level entries in the config files are:
* `api`
* `working_dir`
* `templates` _optional_
* `sources`
* `threads` _optional_
* `messaging`  _optional_
* `video` _optional_

### 1.1. `threads`
If you are running on a cpu which has multiple cores, you can set for example `threads: 2` to run two threads.
Don't use too many threads, you should consider max of `cpu cores * 2`.
Default is `0`.

### 1.2. `api`
`api` contains the `server-mode` settings. To run `m3u-filter` in `server-mode` you need to start it with the `-s`cli argument.
-`api: {host: localhost, port: 8901, web_root: ./web}`

### 1.3. `working_dir`
`working_dir` is the directory where files are written which are given with relative paths.
-`working_dir: ./data`

With this configuration, you should create a `data` directory where you execute the binary.

### 1.4 `templates`
If you have a lot of repeats in you regexps, you can use `templates` to make your regexps cleaner.
You can reference other templates in templates with `!name!`.
```yaml
templates:
  - {name: delimiter, value: '[\s_-]*' }
  - {name: quality, value: '(?i)(?P<quality>HD|LQ|4K|UHD)?'}
```
With this definition you can use `delimiter` and `quality` in your regexp's surrounded with `!` like.

`^.*TF1!delimiter!Series?!delimiter!Films?(!delimiter!!quality!)\s*$`

This will replace all occurrences of `!delimiter!` and `!quality!` in the regexp string.

### 1.5. `sources`
`sources` is a sequence of source definitions, which have two top level entries:
-`inputs`
-`targets`

### 1.5.1 `inputs`
`inputs` is a list of sources.

Each input has the following attributes:

  - `type` is optional, default is `m3u`. Valid values are `m3u` and `xtream`
  - `enabled` is optional, default is true, if you disable the processing is skipped 
  - `persist` is optional, you can skip or leave it blank to avoid persisting the input file. The `{}` in the filename is filled with the current timestamp.
  - `url` for type `m3u` is the download url or a local filename of the input-source. For type `xtream`it is `http://<hostname>:<port>`
  - `epg_url` _optional_ xmltv url
  - `headers` is optional, used only for type `xtream`
  - `username` only mandatory for type `xtream`
  - `pasword`only mandatory for type `xtream`
  - `prefix` is optional, it is applied to the given field with the given value
  - `suffix` is optional, it is applied to the given field with the given value
  - `options` is optional, 
     + `xtream_info_cache` true or false, vod_info and series_info can be cached to disc to reduce network traffic to provider.

`persist` should be different for `m3u` and `xtream` types. For `m3u` use full filename like `./playlist_{}.m3u`.
For `xtream` use a prefix like `./playlist_`

`prefix` and `suffix` are appended after all processing is done, but before sort.
They have 2 fields:
  - `field` can be `name` , `group`, `title`
  - `value` a static text

Example input config for `m3u`
```yaml
sources:
- inputs:
    - url: 'test-input.m3u'
      epg_url: 'test-epg.xml'
      enabled: false
      persist: 'playlist_1_{}.m3u'
    - url: 'https://raw.githubusercontent.com/iptv-org/iptv/master/streams/ad.m3u'
    - url: 'https://raw.githubusercontent.com/iptv-org/iptv/master/streams/au.m3u'
    - url: 'https://raw.githubusercontent.com/iptv-org/iptv/master/streams/za.m3u'
  targets:
   - name: test
     output:
       - type: m3u
         filename: test.m3u
```

Example input config for `xtream`
```yaml
sources:
  inputs:
    - type: xtream
      persist: 'playlist_1_1{}.m3u'
      headers:
        User-Agent: "Mozilla/5.0 (Linux; Tizen 2.3) AppleWebKit/538.1 (KHTML, like Gecko)Version/2.3 TV Safari/538.1"
        Accept: application/json
        Accept-Encoding: gzip
      url: 'http://localhost:8080'
      username: test
      password: test
```


### 1.5.2 `targets`
Has the following top level entries:
- `enabled` _optional_ default is `true`, if you disable the processing is skipped
- `name` _optional_ default is `default`, if not default it has to be unique, for running selective targets
- `sort`  _optional_
- `output` _mandatory_ list of output formats
- `processing_order` _optional_ default is `frm`
- `options` _optional_
- `filter` _mandatory_,
- `rename` _optional_
- `mapping` _optional_
- `watch` _optional_

### 1.5.2.1 `sort`
Has three top level attributes
- `match_as_ascii` _optional_ default is `false`
- `groups`
- `channels`

#### `groups` 
has one top level attribute `order` which can be set to `asc`or `desc`.
#### `channels` 
is a list of sort configurations for groups. Each configuration has 3 top level entries.
- `field` can be  `group`, `title`, `name` or `url`.
- `group_pattern` is a regular expression like `'^TR.:\s?(.*)'` which is matched against group title.
- `order` can be `asc` or `desc`

The pattern should be selected taking into account the processing sequence.

```yml
sort:
  groups:
    order: asc
  channels:
    - { field: name,  group_pattern: '^DE.*',  order: asc }
```

### 1.5.2.2 `output`

Is a list of output format:
Each format has 2 properties 
- `type` 
- `filename`

`type` is _mandatory_  for `m3u`, `strm` and `xtream`.  
`filename` is _mandatory_ if type `m3u` or `strm`, otherwise ignored

`strm` output has additional options 
- `underscore_whitespace`
- `cleanup` 
- `kodi_style`.

`xtream` output has additional options 
- `xtream_skip_live_direct_source` 
- `xtream_skip_video_direct_source`

```yaml
output:
  - type: m3u
    filename: {}.m3u
```

### 1.5.2.3 `processing_order`
The processing order (Filter, Rename and Map) can be configured for each target with:
`processing_order: frm` (valid values are: frm, fmr, rfm, rmf, mfr, mrf. default is frm)

### 1.5.2.4 `options`
- ignore_logo `true` or `false` 
- underscore_whitespace `true` or `false`
- cleanup `true` or `false`
- kodi_style `true` or `false`

`underscore_whitespace`, `cleanup` and `kodi_style` are only valid for `strm` output.

- `ingore_log` logo attributes are ignored to avoid caching logo files on devices.
- `underscore_whitespace` replaces all whitespaces with `_` in the path.
- `cleanup` deletes the directory given at `filename`.
- `kodi_style` tries to rename `filename` with [kodi style](https://kodi.wiki/view/Naming_video_files/TV_shows).

### 1.5.2.5 `filter`
The filter is a string with a filter statement.
The filter can have UnaryExpression `NOT`, BinaryExpression `AND OR`, and Comparison `(Group|Title|Name|Url) ~ "regexp"`.
Filter fields are `Group`, `Title`, `Name` and `Url`.
Example filter:  `((Group ~ "^DE.*") AND (NOT Title ~ ".*Shopping.*")) OR (Group ~ "^AU.*")`

If you use characters like `+ | [ ] ( )` in filters don't forget to escape them!!  

The regular expression syntax is similar to Perl-style regular expressions,
but lacks a few features like look around and backreferences.  
To test the regular expression i use [regex101.com](https://regex101.com/).
Don't forget to select `Rust` option which is under the `FLAVOR` section on the left.

### 1.5.2.6 `rename`
Is a List of rename configurations. Each configuration has 3 top level entries.
- `field` can be  `group`, `title`, `name` or `url`.
- `pattern` is a regular expression like `'^TR.:\s?(.*)'`
- `new_name` can contain capture groups variables addressed with `$1`,`$2`,... 

`rename` supports capture groups. Each group can be addressed with `$1`, `$2` .. in the `new_name` attribute.

This could be used for players which do not observe the order and sort themselves.
```yaml
rename:
  - { field: group,  pattern: ^DE(.*),  new_name: 1. DE$1 }
```
In the above example each entry starting with `DE` will be prefixed with `1.`.

(_Please be aware of the processing order. If you first map, you should match the mapped entries!_)

### 1.5.2.7 `mapping`
`mapping: <list of mapping id's>`
The mappings are defined in a file `mapping.yml`. The filename can be given as `-m` argument.

## Example config file
```yaml
threads: 4
working_dir: ./data
api:
  host: localhost
  port: 8901
  web_root: ./web
templates:
- name: PROV1_TR
  value: >-
    Group ~ "(?i)^.TR.*Ulusal.*" OR
    Group ~ "(?i)^.TR.*Dini.*" OR
    Group ~ "(?i)^.TR.*Haber.*" OR
    Group ~ "(?i)^.TR.*Belgesel.*"
- name: PROV1_DE
  value: >-
    Group ~ "^(?i)^.DE.*Nachrichten.*" OR
    Group ~ "^(?i)^.DE.*Freetv.*" OR
    Group ~ "^(?i)^.DE.*Dokumentation.*"
- name: PROV1_FR
  value: >-
    Group ~ "((?i)FR[:|])?(?i)TF1.*" OR
    Group ~ "((?i)FR[:|])?(?i)France.*"
- name: PROV1_ALL
  value:  "!PROV1_TR! OR !PROV1_DE! OR !PROV1_FR!"
sources:
  - inputs:
      - enabled: true
        url: http://myserver.net/playlist.m3u
        persist: ./playlist_{}.m3u
    targets:
      - name: pl1
        output:
          - type: m3u
            filename: playlist_1.m3u
        processing_order: frm
        options:
          ignore_logo: true
        sort:
          order: asc
        filter: "!PROV1_ALL!" 
        rename:
          - field: group
            pattern: ^DE(.*)
            new_name: 1. DE$1
      - name: pl1strm
        enabled: false
        output:
          - type: strm
            filename: playlist_strm
        options:
          ignore_logo: true
          underscore_whitespace: false
          kodi_style: true
          cleanup: true
        sort:
          order: asc
        filter: "!PROV1_ALL!"
        mapping:
           - France
        rename:
          - field: group
            pattern: ^DE(.*)
            new_name: 1. DE$1
```

### 1.5.2.8 `watch`
For each target with a *unique name*, you can define a watched groups.
It is a list of final group names from this target playlist. 
Final means in this case: the name in the resulting playlist after applying all steps
of transformation.

For example given the following configuration:
```yaml
watch:
  - 'FR | Movies'
  - 'FR | Series'
```

Changes from this groups will be printed as info on console and send to 
the configured messaging (f.e. telegram channel).

### 1.6 `messaging`
`messaging` is an optional configuration for receiving messages.
Currently only telegram is supported.

Messaging is Opt-In, you need to set the `notify_on` message types which are
- `info`
- `stats`
- `error`

```yaml
messaging:
  notify_on:
    - info
    - stats
    - error
  telegram:
    bot_token: '<telegram bot token>'
    chat_ids:
      - '<telegram chat id>'
```

For more information: [Telegram bots](https://core.telegram.org/bots/tutorial)

### 1.7 `video`
`video` is optional.

It has 2 entries `extensions` and `download`.

- `extensions` are a list of video file extensions like `mp4`, `avi`, `mkv`.  
When you have input `m3u` and output `xtream` the url's with the matching endings will be categorized as `video`.

- `download` is _optional_ and is only necessary if you want to download the video files from the ui 
to a specific directory. if defined, the download button from the `ui` is available.
  - `headers` _optional_, download headers
  - `organize_into_directories` _optional_, orgainize downloads into directories  
  - `episode_pattern` _optional_ if you download episodes, the suffix like `S01.E01` should be removed to place all 
files into one folder. The named capture group `episode` is mandatory.  
Example: `.*(?P<episode>[Ss]\\d{1,2}(.*?)[Ee]\\d{1,2}).*`
- `web_search` is _optional_, example: `https://www.imdb.com/search/title/?title={}`, 
define `download.episode_pattern` to remove episode suffix from titles. 

```yaml
video:
  web_search: 'https://www.imdb.com/search/title/?title={}'
  extensions:
    - mkv
    - mp4
    - avi
  download:
    headers:
      User-Agent: "AppleTV/tvOS/9.1.1."
      Accept: "video/*"
    directory: /tmp/
    organize_into_directories: true
    episode_pattern: '.*(?P<episode>[Ss]\\d{1,2}(.*?)[Ee]\\d{1,2}).*'
```

### 1.7 `schedule`
Schedule is optional.
Format is
```yaml
#   sec  min   hour   day of month   month   day of week   year
schedule: "0  0  8,20  *  *  *  *"
```

At the given times the complete processing is started. Do not start it every second or minute.
You could be banned from your server. Twice a day should be enough.

## 2. `mapping.yml`
Has the root item `mappings` which has the following top level entries:
- `templates` _optional_
- `tags` _optional_
- `mapping` _mandatory_

### 2.1 `templates`
If you have a lot of repeats in you regexps, you can use `templates` to make your regexps cleaner.
You can reference other templates in templates with `!name!`;
```yaml
templates:
  - {name: delimiter, value: '[\s_-]*' }
  - {name: quality, value: '(?i)(?P<quality>HD|LQ|4K|UHD)?'}
```
With this definition you can use `delimiter` and `quality` in your regexp's surrounded with `!` like.

`^.*TF1!delimiter!Series?!delimiter!Films?(!delimiter!!quality!)\s*$`

This will replace all occurrences of `!delimiter!` and `!quality!` in the regexp string.

### 2.2 `tags`
Has the following top level entries:
- `name`: unique name of the tag.
- `captures`: List of captured variable names like `quality`. The names should be equal to the regexp capture names.
- `concat`: if you have more than one captures defined this is the join string between them
- `suffix`: suffix for the tag
- `prefix`: prefix for the tag

### 2.3 `mapping`
Has the following top level entries:
- `id` _mandatory_
- `match_as_ascii` _optional_ default is `false`
- `mapper` _mandatory_

### 2.3.1 `id`
Is referenced in the `config.yml`, should be a unique identifier

### 2.3.2 `match_as_ascii`
If you have non ascii characters in you playlist and want to 
write regexp without considering chars like `é` and use `e` instead, set this option to `true`.
[unidecode](https://crates.io/crates/unidecode) is used to convert the text.


### 2.3.3 `mapper`
Has the following top level entries:
- `filter` _optional_
- `pattern`
- `attributes`
- `suffix`
- `prefix`
- `assignments`

#### 2.3.4.1 `filter`
The filter  is a string with a statement (@see filter statements).
It is optional and allows you to filter the content.

#### 2.3.4.2 `pattern`
The pattern is a string with a statement (@see filter statements).
The pattern can have UnaryExpression `NOT`, BinaryExpression `AND OR`, and Comparison `(Group|Title|Name|Url) ~ "regexp"`.
Filter fields are `Group`, `Title`, `Name` and `Url`.
Example filter:  `NOT Title ~ ".*Shopping.*"`

The pattern for the mapper works different from a filter expression.
A filter evaluates the complete expression and returns a result.
The mapper pattern evaluates the expression, but matches directly comparisons and processes them immediately.
To avoid misunderstandings, keep the pattern simply to comparisons.

The regular expression syntax is similar to Perl-style regular expressions,
but lacks a few features like look around and backreferences.

#### 2.3.4.3 `attributes`
Attributes is a map of key value pairs. Valid keys are:
- `id`
- `chno`
- `name`
- `group`
- `title`
- `logo`
- `logo_small`
- `parent_code`
- `audio_track`
- `time_shift`
- `rec`
- `source`

If the regexps matches, the given fields will be set to the new value
You can use `captures` in attributes.
For example you want to `rewrite` the `base_url` for channels in a specific group.

```yaml

mappings:
  templates:
    - name: sports
      value: 'Group ~ ".*SPORT.*"'
    - name: source
      value: 'Url ~ "https?:\/\/(.*?)\/(?P<query>.*)$"'

  mapping:
    - id: sport-mapper
      mapper:
        - filter: '!sports!'
          pattern: "!source!"
          attributes:
            url: http://my.bubble-gum.tv/<query>
```

In this example all channels the urls of all channels with a group name containing `SPORT` will be changed.


#### 2.3.4.4 `suffix`
Suffix is a map of key value pairs. Valid keys are
- name
- group
- title

The special text `<tag:tag_name>` is used to append the tag if not empty.
Example:
```
  suffix:
     name: '<tag:quality>'
     title: '-=[<tag:group>]=-'
```

In this example there must be 2 tag definitions `quality` and `group`.

If the regexps matches, the given fields will be appended to field value

#### 2.3.4.5 `prefix`
Suffix is a map of key value pairs. Valid keys are
- name
- group
- title

The special text `<tag:tag_name>` is used to append the tag if not empty
Example:
```
  suffix:
     name: '<tag:quality>'
     title: '-=[<tag:group>]=-'
```

In this example there must be 2 tag definitions `quality` and `group`.

If the regexps matches, the given fields will be prefixed to field value

#### 2.3.4.6 `assignments`
Attributes is a map of key value pairs. Valid keys and values are:
- `id`
- `chno`
- `name`
- `group`
- `title`
- `logo`
- `logo_small`
- `parent_code`
- `audio_track`
- `time_shift`
- `rec`
- `source`

Example configuration is:
```
assignments:
   title: name
```
This configuration sets `title` property to the value of `name`.

### 2.5 Example mapping.yml file.
```yaml
mappings:
    templates:
      - name: delimiter
        value: '[\s_-]*'
      - name: quality
        value: '(?i)(?P<quality>HD|LQ|4K|UHD)?'
      - name: source
        value: 'Url ~ "https?:\/\/(.*?)\/(?P<query>.*)$"'
    tags:
      - name: quality
        captures:
          - quality
        concat: '|'
        prefix: ' [ '
        suffix: ' ]'
    mapping:
      - id: France
        match_as_ascii: true
        mapper:
          - filter: 'Name ~ "^TF.*"'
            pattern: '!source!'
            attributes:
              url: http://my.iptv.proxy.com/<query> 
          - pattern: 'Name ~ "^TF1$"'
            attributes:
              name: TF1
              id: TF1.fr,
              chno: '1',
              logo: https://upload.wikimedia.org/wikipedia/commons/thumb/3/3c/TF1_logo_2013.svg/320px-TF1_logo_2013.svg.png
            suffix:
              title: '<tag:quality>'
              group: '|FR|TNT'
            assignments:
              title: name
          - pattern: 'Name ~ "^TF1!delimiter!!quality!*Series[_ ]*Films$"'
            attributes:
              name: TF1 Series Films,
              id: TF1SeriesFilms.fr,
              chno: '20',
              logo: https://upload.wikimedia.org/wikipedia/commons/thumb/3/3c/TF1_logo_2013.svg/320px-TF1_logo_2013.svg.png,
            suffix:
              group: '|FR|TNT'
```

## 3. Api-Proxy Config
If you use the proxy functionality, 
you need to create a `api-proxy.yml` configuration.

You can specify the path for the file with the  `-a` cli argument. 

`username` and `password`are mandatory for credentials. `username` is unique.
The `token` is _optional_. If defined it should be unique. The `token`can be used
instead of username+password

To access the api for: 
- `xtream` use url like `http://192.169.1.2/player_api.php?username={}&password={}`
- `m3u` use url `http://192.169.1.2/get.php?username={}&password={}`
or with token
- `xtream` use url like `http://192.169.1.2/player_api.php?token={}`
- `m3u` use url `http://192.169.1.2/get.php?token={}`

To access the xmltv-api use url like `http://192.169.1.2/xmltv.php?username={}&password={}`

_Do not forget to replace `{}` with credentials._

If you use the endpoints through rest calls, you can use, for the sake of simplicity:
- `m3u` inplace of `get.php`
- `xtream` inplace of `player_api.php`
- `epg` inplace of `xmltv.php`
- `token` inplace of `username` and `password` combination

When you define credentials for a `target`, ensure that this target has
`output` format  `xtream`or `m3u`.


```yaml
server:
  protocol: http
  ip: 192.168.0.3
  http_port: 80
  https_port: 443
  rtmp_port: 0
  timezone: Europe/Paris
  message: Welcome to m3u-filter
user:
  - target: pl1
    credentials:
      - {username: x3452, password: ztrhgrGZrt83hjerter, token: 4342sdfr3424}
```


## 4. Logging
Following log levels are supported:
- `debug`
- `info` _default_
- `warn`
- `error`
 
Use the `-l` or `-log-level` cli-argument to specify the log-level. 

## 6. Web-UI

![m3u-filter-tree](https://github.com/euzu/m3u-filter/assets/33094714/0455d598-1953-4b69-b9ab-d741e81f0031)
![m3u-filter-prefs](https://github.com/euzu/m3u-filter/assets/33094714/9763c11a-fc12-4e0b-93f5-6f05546dd628)

## 6. Compilation

### Static binary for docker

#### `cross`compile

Ease way to compile is a docker toolchain `cross`

```sh
rust install cross
env  RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target x86_64-unknown-linux-musl
```

#### Manual compile - install prerequisites
```
rustup update
sudo apt-get install pkg-config musl-tools libssl-dev
rustup target add x86_64-unknown-linux-musl
```
#### Build statically linked binary
```
cargo build --target x86_64-unknown-linux-musl --release
```
#### Dockerize
Dockerfile
```
FROM gcr.io/distroless/base-debian12 as build

FROM scratch

WORKDIR /

COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

COPY ["./m3u-filter", "./config.yml",  "./api-proxy.yml",  "./mapping.yml", "/"]
COPY ./web /web

CMD ["/m3u-filter", "-s", "-c", "/config.yml"]
```
Image
```
docker build -t m3u-filter .
```
docker-compose.yml
```
version: '3'
services:
  m3u-filter:
    container_name: m3u-filter
    image: m3u-filter:latest
    working_dir: /
    volumes:
      - ./data:/data
    ports:
      - "8901:8901"
    environment:
      - TZ=Europe/Paris
    restart: unless-stopped
```

The image should be around 15MB.
```
m3u-filter$ docker images
REPOSITORY                             TAG       IMAGE ID       CREATED        SIZE
m3u-filter                             latest    c59e1edb9e56   1 day ago     14.6MB
```

### Cross compile for windows on linux
If you want to compile this project on linux for windows, you need to do the following steps.

#### Install mingw packages for your distribution
For ubuntu type:
```shell
sudo apt-get install gcc-mingw-w64
```
#### Install mingw support for rust
```shell
rustup target add x86_64-pc-windows-gnu
rustup toolchain install stable-x86_64-pc-windows-gnu
```

Compile it with:
```sh
cargo build --release --target x86_64-pc-windows-gnu
```

### Cross compile for raspberry pi 2/3/4

Ease way to compile is a docker toolchain `cross`

```sh
rust install cross
env  RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target armv7-unknown-linux-musleabihf
```


