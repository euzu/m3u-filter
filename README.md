# m3u-filter

[Wiki](https://github.com/euzu/m3u-filter/wiki)

m3u-filter is a simple application which can:
  - filter, rename, map and sort entries out of a playlist and persist in EXTM3U, XTREAM or Kodi format.
  - can process multiple inputs and can create multiple outputs from this input files trough target definitions.
  - act as simple xtream or m3u server after processing entries
  - act as `redirect` or `reverse` proxy for xtream 
  - can schedule updates in server mode
  - can run as cli-command for serving processed playlists through web-server like nginx or apache.
  - can define multiple targets for filtering if you want to create multiple playlists from a big playlist.
  - use regular expressions for matching
  - define filter as statements like `filter: (Group ~ "^FR.*") AND NOT(Group ~ ".*XXX.*" OR Group ~ ".*SERIES.*" OR Group ~".*MOVIES.*")`
  - DRY - define templates and use them, don't repeat yourself
  - Send a telegram bot or rest message when something goes wrong
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

## Command line Arguments
```
Usage: m3u-filter [OPTIONS]

Options:
  -p, --config-path <CONFIG_PATH>  The config directory
  -c, --config <CONFIG_FILE>       The config file
  -i, --source <SOURCE_FILE>       The source config file
  -m, --mapping <MAPPING_FILE>     The mapping file
  -t, --target <TARGET>            The target to process
  -a, --api-proxy <API_PROXY>      The user file
  -s, --server                     Run in server mode
  -l, --log-level <LOG_LEVEL>      log level
  -h, --help                       Print help
  -V, --version                    Print version
  --genpwd                         Generate UI Password
  --healthcheck                    Healtcheck for docker
```

## 1. `config.yml`

For running in cli mode, you need to define a `config.yml` file which can be xonfig directory next to the executable or provided with the
`-c` cli argument.

For running specific targets use the `-t` argument like `m3u-filter -t <target_name> -t <other_target_name>`.
Target names should be provided in the config. The -t option overrides `enabled` attributes of `input` and `target` elements.
This means, even disabled inputs and targets are processed when the given target name as cli argument matches a target.

Top level entries in the config files are:
* `api`
* `working_dir`
* `threads` _optional_
* `messaging`  _optional_
* `video` _optional_
* `schedule` _optional_
* `backup_dir` _optional_
* `update_on_boot` _optional_
* `web_ui_enabled` _optional_
* `web_auth` _optional_

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

### 1.4 `messaging`
`messaging` is an optional configuration for receiving messages.
Currently only  and rest is supported.

Messaging is Opt-In, you need to set the `notify_on` message types which are
- `info`
- `stats`
- `error`

`telegram` and `rest` configurations are optional.

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
  rest:
    url: '<api url as POST endpoint for json data>'
```

For more information: [Telegram bots](https://core.telegram.org/bots/tutorial)

### 1.5 `video`
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

### 1.5 `schedule`
Schedule is optional.
Format is
```yaml
#   sec  min   hour   day of month   month   day of week   year
schedule: "0  0  8,20  *  *  *  *"
```

At the given times the complete processing is started. Do not start it every second or minute.
You could be banned from your server. Twice a day should be enough.

### 1.6 `backup_dir`
is the directory where the backup configuration files written, when saved from the ui.

### 1.7 `update_on_boot`
if set to true, an update is started when the application starts.

### 1.8 `web_ui_enabled`
default is true, if set to false the web_ui is disabled

### 1.9 `web_auth`
Web UI Authentication can be enabled if `web_ui_enabled` is `true`.

```yaml
web_ui_enabled: true
web_auth:
  enabled: true
  secret: very.secret.secret
  issuer: m3u_filter
  userfile: user.txt
```

- `web_auth` can be deactivated if `enabled` is set to `false`. If not set default is `true`.
- `secret` is used for jwt token generation.
- `userfile` is the file where the ui users are stored. if the filename is not absolute `m3u-filter` will look into the `config_dir`. if `userfile`is not given the default value is `user.txt`

You can generate a secret for jwt token for example with `node -e "console.log(require('crypto').randomBytes(32).toString('hex'))"`

The userfile has the format  `username: password` per line.
Example:
```
test: $argon2id$v=19$m=19456,t=2,p=1$QUpBWW5uellicTFRUU1tR0RVYVVEUTN5UEJDaWNWQnI3Rm1aNU1xZ3VUSWc3djZJNjk5cGlkOWlZTGFHajllSw$3HHEnLmHW07pjE97Inh85RTi6VN6wbV27sT2hHzGgXk
nobody: $argon2id$v=$argon2id$v=19$m=19456,t=2,p=1$Y2FROE83ZDQ1c2VaYmJ4VU9YdHpuZ2c2ZUwzVkhlRWFpQk80YVhNMEJCSlhmYk8wRE16UEtWemV2dk81cmNaNw$BB81wmEm/faku/dXenC9wE7z0/pt40l4YGh8jl9G2ko
```

The password can be generated with
```shell
./m3u-filter  -p /op/m3u-filter/config --genpwd`
```

or with docker
```shell
docker container exec -it m3u-filter ./m3u-filter --genpwd
```

The encrypted pasword needs to be added manually into the users file.

## Example config file
```yaml
threads: 4
working_dir: ./data
api:
  host: localhost
  port: 8901
  web_root: ./web
```

## 2. `source.yml`

Has the following top level entries:
* `templates` _optional_
* `sources`

### 2.1 `templates`
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

### 2.2. `sources`
`sources` is a sequence of source definitions, which have two top level entries:
-`inputs`
-`targets`

### 2.2.1 `inputs`
`inputs` is a list of sources.

Each input has the following attributes:

- `name` is optional, if set it must be unique, should be set for the webui
- `type` is optional, default is `m3u`. Valid values are `m3u` and `xtream`
- `enabled` is optional, default is true, if you disable the processing is skipped
- `persist` is optional, you can skip or leave it blank to avoid persisting the input file. The `{}` in the filename is filled with the current timestamp.
- `url` for type `m3u` is the download url or a local filename of the input-source. For type `xtream`it is `http://<hostname>:<port>`
- `epg_url` _optional_ xmltv url
- `headers` is optional
- `username` only mandatory for type `xtream`
- `pasword`only mandatory for type `xtream`
- `prefix` is optional, it is applied to the given field with the given value
- `suffix` is optional, it is applied to the given field with the given value
- `options` is optional,
    + `xtream_info_cache` true or false, vod_info and series_info can be cached to disc to reduce network traffic to provider.
    + `xtream_skip_live` true or false, live section can be skipped.
    + `xtream_skip_vod` true or false, vod section can be skipped. 
    + `xtream_skip_series` true or false, series section can be skipped.


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
      name: test_m3u
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
        User-Agent: "Mozilla/5.0 (AppleTV; U; CPU OS 14_2 like Mac OS X; en-us) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0.1 Safari/605.1.15"
        Accept: application/json
        Accept-Encoding: gzip
      url: 'http://localhost:8080'
      username: test
      password: test
```


### 2.2.2 `targets`
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

### 2.2.2.1 `sort`
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

```yaml
sort:
  groups:
    order: asc
  channels:
    - { field: name,  group_pattern: '^DE.*',  order: asc }
```

### 2.2.2.2 `output`

Is a list of output format:
Each format has 2 properties
- `type`
- `filename`

`type` is _mandatory_  for `m3u`, `strm` and `xtream`.  
`filename` is _mandatory_ if type is `strm`. if type is `m3u` the plain m3u file is written but it is not used by `m3u-filter`.

`strm` output has additional options
- `underscore_whitespace`
- `cleanup`
- `kodi_style`.

`xtream` output has additional options
- `xtream_skip_live_direct_source`  if true the direct_source property from provider for live is ignored
- `xtream_skip_video_direct_source`  if true the direct_source property from provider for movies is ignored
- `xtream_skip_series_direct_source`  if true the direct_source property from provider for series is ignored

`m3u` output has additional options
Because xtream api delivers only the metadata to series, we need to fetch the series and resolve them. But be aware,
each series info entry needs to be fetched one by one. 
- `xtream_resolve_series` if is set to `true` and you have xtream input and m3u output, the series are fetched and resolved.
This can cause a lot of requests to the provider. Be cautious when using this option.  
- `xtream_resolve_series_delay` to avoid a provider ban you can set the seconds between series_info_request's. Default is 2 seconds.
But be aware that the more series entries there are, the longer the process takes. 

```yaml
output:
  - type: m3u
    filename: playlist.m3u
```

### 2.2.2.3 `processing_order`
The processing order (Filter, Rename and Map) can be configured for each target with:
`processing_order: frm` (valid values are: frm, fmr, rfm, rmf, mfr, mrf. default is frm)

### 2.2.2.4 `options`
- ignore_logo `true` or `false`
- underscore_whitespace `true` or `false`
- cleanup `true` or `false`
- kodi_style `true` or `false`

`underscore_whitespace`, `cleanup` and `kodi_style` are only valid for `strm` output.

- `ingore_log` logo attributes are ignored to avoid caching logo files on devices.
- `underscore_whitespace` replaces all whitespaces with `_` in the path.
- `cleanup` deletes the directory given at `filename`.
- `kodi_style` tries to rename `filename` with [kodi style](https://kodi.wiki/view/Naming_video_files/TV_shows).

### 2.2.2.5 `filter`
The filter is a string with a filter statement.
The filter can have UnaryExpression `NOT`, BinaryExpression `AND OR`, Regexp Comparison `(Group|Title|Name|Url) ~ "regexp"`
and Type Comparsison `Type = vod` or `Type = live` or `Type = series`.
Filter fields are `Group`, `Title`, `Name`, `Url` and `Type`.
Example filter:  `((Group ~ "^DE.*") AND (NOT Title ~ ".*Shopping.*")) OR (Group ~ "^AU.*")`

If you use characters like `+ | [ ] ( )` in filters don't forget to escape them!!

The regular expression syntax is similar to Perl-style regular expressions,
but lacks a few features like look around and backreferences.  
To test the regular expression i use [regex101.com](https://regex101.com/).
Don't forget to select `Rust` option which is under the `FLAVOR` section on the left.

### 2.2.2.6 `rename`
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

### 2.2.2.7 `mapping`
`mapping: <list of mapping id's>`
The mappings are defined in a file `mapping.yml`. The filename can be given as `-m` argument.

## Example source.yml file
```yaml
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

### 2.5.2.8 `watch`
For each target with a *unique name*, you can define watched groups.
It is a list of regular expression matching final group names from this target playlist. 
Final means in this case: the name in the resulting playlist after applying all steps
of transformation.

For example given the following configuration:
```yaml
watch:
  - 'FR - Movies \(202[34]\)'
  - 'FR - Series'
```

Changes from this groups will be printed as info on console and send to
the configured messaging (f.e. telegram channel).

To get the watch notifications over messaging notify_on `watch` should be enabled.  
In `config.yml`
```yaml
messaging:
  notify_on:
    - watch
```

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
- `epg_channel_id` or `epg_id`
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
- `url`

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
```yaml
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
```yaml
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
```yaml
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
              id: TF1.fr
              chno: '1'
              logo: https://upload.wikimedia.org/wikipedia/commons/thumb/3/3c/TF1_logo_2013.svg/320px-TF1_logo_2013.svg.png
            suffix:
              title: '<tag:quality>'
              group: '|FR|TNT'
            assignments:
              title: name
          - pattern: 'Name ~ "^TF1!delimiter!!quality!*Series[_ ]*Films$"'
            attributes:
              name: TF1 Series Films
              id: TF1SeriesFilms.fr
              chno: '20'
              logo: https://upload.wikimedia.org/wikipedia/commons/thumb/3/3c/TF1_logo_2013.svg/320px-TF1_logo_2013.svg.png,
            suffix:
              group: '|FR|TNT'
```

## 3. Api-Proxy Config
If you use the proxy functionality, 
you need to create a `api-proxy.yml` configuration.

You can specify the path for the file with the  `-a` cli argument. 

The configuration contains the server info for xtream accounts and user definitions.
You can define multiple server with unique names, one should be named `default`.

Iptv player can act differently and use the direct-source attribute or can compose the url based on the server info.
The options `xtream_skip_live_direct_source`, `xtream_skip_video_direct_source` and `xtream_skip_series_direct_source` are default `true` to avoid this problem. 
You can set them fo `false`to keep the direct-source attribute.

`username` and `password`are mandatory for credentials. `username` is unique.
The `token` is _optional_. If defined it should be unique. The `token`can be used
instead of username+password
`proxy` is _optional_. If defined it can be `reverse` or `redirect`. Default is `redirect`.
`server` is _optional_. It should match one server definition, if not given the server with the name `default` is used or the first one.  

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

The `proxy` property can be `reverse`or `redirect`. `reverse` means the streams are going through m3u-filter, `redirect` means the streams are comming from your provider.

If you use `https` you need a ssl terminator. `m3u-filter` does not support https traffic. 

```yaml
server:
  - name: default 
    protocol: http
    host: 192.168.0.3
    http_port: 80
    https_port: 443
    rtmp_port: 0
    timezone: Europe/Paris
    message: Welcome to m3u-filter
  - name: external
    protocol: http
    host: my_external_domain.com
    http_port: 80
    https_port: 443
    rtmp_port: 0
    timezone: Europe/Paris
    message: Welcome to m3u-filter
user:
  - target: pl1
    credentials:
      - {username: x3452, password: ztrhgrGZ, token: 4342sd, proxy: reverse, server: external}
      - {username: x3451, password: secret, token: abcde, proxy: redirect}
```


## 4. Logging
Following log levels are supported:
- `debug`
- `info` _default_
- `warn`
- `error`
 
Use the `-l` or `--log-level` cli-argument to specify the log-level.

The log level can be set through environment variable `M3U_FILTER_LOG`.

Precedence has cli-argument.

Log Level has module support like `m3u_filter::util=error,m3u_filter::filter=debug,m3u_filter=debug`

## 6. Web-UI

![m3u-filter-tree](https://github.com/euzu/m3u-filter/assets/33094714/0455d598-1953-4b69-b9ab-d741e81f0031)
![m3u-filter-prefs](https://github.com/euzu/m3u-filter/assets/33094714/9763c11a-fc12-4e0b-93f5-6f05546dd628)

## 6. Compilation

### Docker build
Change into the root directory and run:

```shell
docker build --rm -f docker/Dockerfile -t m3u-filter .  
```

This will build the complete project and create a docker image.

To start the container, you can use the `docker-compose.yml`
But you need to change `image: ghcr.io/euzu/m3u-filter:latest` to `image: m3u-filter`


### Manual build static binary for docker

#### `cross`compile

Ease way to compile is a docker toolchain `cross`

```shell
rust install cross
env  RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target x86_64-unknown-linux-musl
```

#### Manual compile - install prerequisites
```shell
rustup update
sudo apt-get install pkg-config musl-tools libssl-dev
rustup target add x86_64-unknown-linux-musl
```
#### Build statically linked binary
```shell
cargo build --target x86_64-unknown-linux-musl --release
```
#### Dockerize
Dockerfile
```dockerfile
FROM gcr.io/distroless/base-debian12 as build

FROM scratch

WORKDIR /

COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

COPY ./m3u-filter /
COPY ./web /web

CMD ["/m3u-filter", "-s", "-p", "/config"]
```
Image
```shell
docker build -t m3u-filter .
```
docker-compose.yml
```dockerfile
version: '3'
services:
  m3u-filter:
    container_name: m3u-filter
    image: m3u-filter
    user: "133:144"
    working_dir: /
    volumes:
      - ./config:/config
      - ./data:/data
      - ./backup:/backup
      - ./downloads:/downloads
    environment:
      - TZ=Europe/Paris
    ports:
      - "8901:8901"
    restart: unless-stopped
```

If you want to use m3u-filter with docker-compose, there is a `--healthcheck` argument for healthchecks

```dockerfile
    healthcheck:
      test: ["CMD", "/m3u-filter", "-p", "/config" "--healthcheck"]  
      interval: 30s  
      timeout: 10s   
      retries: 3     
      start_period: 10s
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
```shell
cargo build --release --target x86_64-pc-windows-gnu
```

### Cross compile for raspberry pi 2/3/4

Ease way to compile is a docker toolchain `cross`

```shell
rust install cross
env  RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target armv7-unknown-linux-musleabihf
```

# Different Scenarios
## Using `m3u-filter` with a m3u provider.
 todo.

## Using `m3u-filter` with a xtream provider.

You have a provider who supports the xtream api.

The provider gives you:
- the url: `http://fantastic.provider.xyz:8080`
- username: `tvjunkie`
- password: `junkie.secret`
- epg_url: `http://fantastic.provider.xyz:8080/xmltv.php?username=tvjunkie&password=junkie.secret`


To use `m3u-filter` you need to create the configuration.
The configuration consist of 4 files.
- config.yml
- source.yml
- mapping.yml
- api-proxy.yml

The file `mapping.yml`is optional and only needed if you want to do something linke renaming titles or changing attributes.

Lets start with `config.yml`. An example basic configuration is:

```yaml
api: {host: 0.0.0.0, port: 8901, web_root: ./web}
working_dir: ./data
update_on_boot: true
```

This configuration starts `m3u-filter`and listens on the 8901 port. The downloaded playlists are stored inside the `data`-folder in the current working directory.
The property `update_on_boot` is optional and can be helpful in the beginning until you have found a working configuration. I prefer to set it to false.

Now we have to define the sources we want to import. We do this inside `source.yml`

```yaml
templates:
- name: ALL_CHAN
  value: 'Group ~ ".*"'
sources:
- inputs:
    - type: xtream
      url: 'http://fantastic.provider.xyz:8080'
      epg_url: 'http://fantastic.provider.xyz:8080/xmltv.php?username=tvjunkie&password=junkie.secret'
      username: tvjunkie
      password: junkie.secret
      options: {xtream_info_cache: true}
  targets:
    - name: all_channels
      output:
        - type: xtream
      filter: "!ALL_CHAN!"
      options: {ignore_logo: false, xtream_skip_live_direct_source: true, xtream_skip_video_direct_source: true}
      sort:
        match_as_ascii: true
        groups:
          order: asc
```

What did we do? First, we defined the input source based on the information we received from our provider.
Then we defined a target that we will create from our source.
This configuration creates a 1:1 copy (this is probably not what we want, but we discuss the filtering later).

Now we need to define the user access to the created target. We need to define `api-proxy.yml`.

```yaml
server:
- name: default
  protocol: http
  host: 192.168.1.41
  http_port: '8901'
  timezone: Europe/Berlin
  message: Welcome to m3u-filter
- name: external
  protocol: https
  host: tvjunkie.dyndns.org
  http_port: '80'
  https_port: '443'
  rtmp_port: '1953'
  timezone: Europe/Berlin
  message: Welcome to m3u-filter
user:
- target: all_channels
  credentials:
  - username: xt
    password: xt.secret
    proxy: redirect
    server: default
  - username: xtext
    password: xtext.secret
    proxy: redirect
    server: external
```
We have defined 2 server configurations. The `default` configuration is intended for use in the local network, the IP address is that of the computer on which `m3u-filter` is running. The `external` configuration is optional and is only required for access from outside your local network. External access requires port forwarding on your router and an SSL terminator proxy such as nginx and a dyndns provider configured from your router if you do not have a static IP address (this is outside the scope of this manual).

The next section of the `api-proxy.yml` contains the user definition. We can define users for each `target` from the `source.yml`.
This means that each `user` can only access one `target` from `source.yml`.  We have named our target `all_channels` in `source.yml` and used this name for the user definition.  We have defined 2 users, one for local access and one for external access.
We have set the proxy type to `redirect`, which means that the client will be redirected to the original provider URL when opening a stream. If you set the proxy type to `reverse`, the stream will be streamed from the provider through `m3u-filter`. Based on the hardware you are running `m3u-filter` on, you can opt for the proxy type `reverse`. But you should start with `redirect` first until everything works well.


To access a xtream api from our IPTV-application we need at least 3 information  the `url`, `username` and `password`.
All this information are now defined in `api-proxy.yml`.
- url: `http://192.168.1.41:8901`
- username: `xt`
- password: `xt.secret`

Start `m3u-filter`,  fire up your IPTV-Application, enter credentials and watch.

# It works well, but I don't need all the channels, how can I filter?

You need to understand regular expressions to define filters. A good site for learning and testing regular expressions is [regex101.com](https://regex101.com). Don't forget to set FLAVOR on the left side to Rust.

To adjust the filter, you must change the `source.yml` file.
What we have currently is: (for a better overview I have removed some parts and marked them with ...)

```yaml
templates:
- name: ALL_CHAN
  value: 'Group ~ ".*"'
sources:
- inputs:
    - type: xtream
      ...
  targets:
    - name: all_channels
      output:
        - type: xtream
      filter: "!ALL_CHAN!"
      ...
```

We use templates to make the filters easier to maintain and read.

Ok now let's start.

First: We have a lot of channel groups we dont need.

`m3u-filter` excludes or includes groups or channels based on filter. Usable fields for filter are `Group`, `Name` and `Title`.
The simplest filter is:

`<Field> ~ <Regular Expression>`.  For example  `Group ~ ".*"`. This means include all categories.

Ok, if you only want the Shopping categories, here it is: `Group ~ ".*Shopping.*"`. This includes all categories whose name contains shopping.

Wait, we are missing categories that contain 'shopping'. Regular expressions are case-sensitive. You must explicitly define a case-insensitive regexp. `Group ~ "(?i).*Shopping.*"` will match everything containing Shopping, sHopping, ShOppInG,....

But what if i want to reverse the filter? I dont want a shoppping category. How can I achieve this? Quite simply with `NOT`.
`NOT(Group ~ "(?i).*Shopping.*")`. Thats it.


You can combine Filter with `AND` and `OR` to create more complex filter.

For example:
`(Group ~ "^FR.*" AND NOT(Group ~ "^FR.*SERIES.*" OR Group ~ "^DE.*EINKAUFEN.*" OR Group ~ "^EN.*RADIO.*" OR Group ~ "^EN.*ANIME.*"))`

As you can see, this can become very complex and unmaintainable. This is where the templates come into play.

We can disassemble the filter into smaller parts and combine them into a more powerfull filter.

```yaml
templates:
- name: NO_SHOPPING
  value: 'NOT(Group ~ "(?i).*Shopping.*" OR Group ~ "(?i).*Einkaufen.*") OR Group ~ "(?i).*téléachat.*"'
- name: GERMAN_CHANNELS
  value: 'Group ~ "^DE: .*"'
- name: FRENCH_CHANNELS
  value: 'Group ~ "^FR: .*"'
- name: MY_CHANNELS
  value: '!NO_SHOOPING! AND (!GERMAN_CHANNELS! OR !FRENCH_CHANNELS!)'

sources:
- inputs:
    - type: xtream
      ...
  targets:
    - name: all_channels
      output:
        - type: xtream
      filter: "!MY_CHANNELS!"
      ...
```

The resulting playlist contains all French and German channels except Shopping.

Wait, we've only filtered categories, but what if I want to exclude a specific channel?
No Problem. You can write a filter for your channel using the `Name` or `Title` property.
`NOT(Title ~ "FR: TV5Monde")`. If you have this channel in different categories, you can alter your filter like:
`NOT(Group ~ "FR: TF1" AND Title ~ "FR: TV5Monde")`.

```yaml
templates:
- name: NO_SHOPPING
  value: 'NOT(Group ~ "(?i).*Shopping.*" OR Group ~ "(?i).*Einkaufen.*") OR Group ~ "(?i).*téléachat.*"'
- name: GERMAN_CHANNELS
  value: 'Group ~ "^DE: .*"'
- name: FRENCH_CHANNELS
  value: 'Group ~ "^FR: .*"'
- name: NO_TV5MONDE_IN_TF1
  value: 'NOT(Group ~ "FR: TF1" AND Title ~ "FR: TV5Monde")'
- name: EXCLUDED_CHANNELS
  value: '!NO_TV5MONDE_IN_TF1! AND !NO_SHOOPING!'
- name: MY_CHANNELS
  value: '!EXCLUDED_CHANNELS! AND (!GERMAN_CHANNELS! OR !FRENCH_CHANNELS!)'
```

