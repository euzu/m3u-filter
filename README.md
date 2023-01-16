# m3u-filter

m3u-filter is a simple application which can filter, rename and map entries out of a playlist in EXTM3U format.
If you have a playlist which contains unwanted entries, you can create filter which include or discard entries
based on the header information of the playlist entries, you can rename entries or map entries based on regular expressions.
Currently filter and rename operations support group, name and title fields.

You can run m3u-filter as command line application to update your playlists (manually or as cron job), or you can 
run it in server mode and open the web-ui to see the contents of the playlist, filter/search content and save the filtered groups as a new playlist.

m3u-filter can process multiple inputs and can create multiple files from this input files trough target definitions. 
You can define multiple targets for filtering if you want to create multiple playlists from a big playlist.

## Starting in server mode for Web-UI
If you want to see the contents of a playlist, you can simply start with the `-s` (`--server`)
argument. Other arguments are ignored. A server is started. You can open a browser to view the Web-UI.
According to your configuration, use the printed url on console.
The UI allows you to download a list. You can download the list with the Save Button.
The downloaded list only contains *non-selected* entries.

## 1. `config.yml`

For running in cli mode, you need to define a `config.yml` file which can be next to the executable or provided with the
`-c` cli argument. It contains the filter, rename and mapping definitions.

Top level entries in the config files are:
* api
* working_dir
* sources

### 1.1. `api`
`api` contains the `server-mode` settings. To run `m3u-filter` in `server-mode` you need to start it with the `s`cli argument.
* `api: {host: localhost, port: 8901, web_root: ./web}`

### 1.2. `working_dir`
`working_dir` is the directory where file are written which are given with relative paths.
* `working_dir: ./data`

With this configuration, you should create a `data` directory next to the executable.

### 1.3. `sources`
`sources` is a sequence of source definitions, which have two top level entries:
 * `input`
 * `targets`

### 1.3.1 `input`
Has two entries, `persist` and `url`.

`input: { persist: ./playlist_{}.m3u, url: http://myserver.net/playlist.m3u }`

  - `persist` is optional, you can skip or leave it blank to avoid persisting the input file. The `{}` in the filename is filled with the current timestamp.
  - `url` is the download url or a local filename of the input-source.

### 1.3.2 `targets`
Has the following top level entries:
* `filename` _mandatory_
* `sort`  _optional_
* `output` _optional_ default is `M3u`
* `processing_order` _optional_ default is `FRM`
* `options` _optional_
* `filter` _mandatory_,
* `rename` _optional_
* `mapping` _optional_

### 1.3.2.1 `filename`
Is the filename for the resulting playlist.

### 1.3.2.2 `sort`
Has one top level attribute `order` which can be set to `Asc`or `Desc`.

### 1.3.2.3 `output`
There are two types of targets ```M3u``` and ```Strm```. 
If the attribute is not specified ```M3u``` is created by default.
You can set options for each `output` type.

`Strm` output has additional options `underscore_whitespace`, `cleanup` and `kodi_style`.

### 1.3.2.4 `processing_order`
The processing order (Filter, Rename and Map) can be configured for each target with:
`processing_order: FRM` (valid values are: FRM, FMR, RFM, RMF, MFR, MRF. default is FRM)

### 1.3.2.5 `options`
* ignore_logo `true` or `false` 
* underscore_whitespace `true` or `false`
* cleanup `true` or `false`
* kodi_style `true` or `false`

`underscore_whitespace`, `cleanup` and `kodi_style` are only valid for `Strm` output.

- `ingore_log` logo attributes are ignored to avoid caching logo files on devices.
- `underscore_whitespace` replaces all whitespaces with `_` in the path.
- `cleanup` deletes the directory given at `filename`.
- `kodi_style` tries to rename `filename` with [kodi style](https://kodi.wiki/view/Naming_video_files/TV_shows).

### 1.3.2.6 `filter`
The filter is a string with a filter statement.
The filter can have UnaryExpression `NOT`, BinaryExpression `AND OR`, and Comparison `(Group|Title|Name|Url) ~ "regexp"`.
Filter fields are `Group`, `Title`, `Name` and `Url`.
Example filter:  `((Group ~ "^DE.*") AND (NOT Title ~ ".*Shopping.*")) OR (Group ~ "^AU.*")`

The regular expression syntax is similar to Perl-style regular expressions,
but lacks a few features like look around and backreferences.

### 1.3.2.7 `rename`
Has 3 top level entries.
* `field`  `Group`, `Title`, `Name` or `Url`.
* `new_name` can contain capture groups variables adressed with `$1`,`$2`,... 

`rename` supports capture groups. Each group can be adressed with `$1`, `$2` .. in the `new_name` attribute.

This could be used for players which do not observe the order and sort themselves.
```yaml
rename:
  - { field: Group,  pattern: ^DE(.*),  new_name: 1. DE$1 }
```
In the above example each entry starting with `DE` will be prefixed with `1.`.

(_Please be aware of the processing order. If you first map, you should match the mapped entries!_)

### 1.3.2.8 `mapping`
`mapping: <list of mapping id's>`
The mappings are defined in a file `mapping.yml`. The filename can be given as `-m` argument.

## Example config file
```yaml
working_dir: ./data
api:
  host: localhost
  port: 8901
  web_root: ./web
input:
  url: http://myserver.net/playlist.m3u
  persist: ./playlist_{}.m3u
targets:
  - filename: playlist_1.m3u
    processing_order: FRM
    options:
      ignore_logo: true
    sort:
      order: Asc
    filter: Group ~ "^DE\s.*" OR Group ~ "^AU\s.*" 
    rename:
      - field: Group
        pattern: ^DE(.*)
        new_name: 1. DE$1
  - filename: playlist_strm
    output: Strm
    options:
      ignore_logo: true
      underscore_whitespace: false
      kodi_style: true
      cleanup: true
    sort:
      order: Asc
    filter: Group ~ "^DE\s.*" OR Group ~ "^AU\s.*"
    mapping:
       - France
    rename:
      - field: Group
        pattern: ^DE(.*)
        new_name: 1. DE$1
```

## 2. `mapping.yml`
Has following top level entries:
* id _mandatory_
* tag _optional_
* match_as_ascii _optional_ default is `false`
* templates _optional_
* mapper _mandatory_

### 2.1 `id`
Is referenced in the `config.yml`, should be a unique identifier

### 2.2 `tag`
Has following top level entries: 
  - `captures`: List of captured variable names like `quality`. The names should be equal to the regexp capture names.
  - `concat`: if you have more than one captures defined this is the join string between them
  - `suffix`: suffix for the tag
  - `prefix`: prefix for the tag

### 2.2 `match_as_ascii`
If you have non ascii characters in you playlist and want to 
write regexp without considering chars like `é` and use `e` instead, set this option to `true`.
[unidecode](https://crates.io/crates/unidecode) is used to convert the text.

### 2.3 `templates`
If you have a lot of repeats in you regexps, you can use `templates` to make your regexps cleaner.
```yaml
templates:
  - {name: delimiter, value: '[\s_-]*' }
  - {name: quality, value: '(?i)(?P<quality>HD|LQ|4K|UHD)?'}
```
With this definition you can use `delimiter` and `quality` in your regexp's surrounded with `!` like.

`^.*TF1!delimiter!Series?!delimiter!Films?(!delimiter!!quality!)\s*$`

This will replace all occurrences of `!delimiter!` and `!quality!` in the regexp string.

### 2.4 `mapper`
Has following top level entries:
* `tvg_name`
* `tvg_names`
* `tvg_id` simple text
* `tvg_chno` simple text
* `tvg_logo` simple text
* `group_title` sequence of simple text which are concatenated with `|` and added to the `group` entry.

#### 2.4.1 `tvg_name`
Is a simple text which can contain captured variable names like `TF1 $quality`. For this example 
you should use `tag` instead, because it prevents the spaces if no matching quality was found.

#### 2.4.2 `tvg_names`
Sequence for regexp's to apply the defined changes on match.

(_Please be aware of the processing order. If you first rename, you should match the renamed entries!_)
```yaml
tvg_names:
  - '^.*TF1!delimiter!Series?!delimiter!Films?(!delimiter!!quality!)\s*$'
```

### 2.5 Example mapping.yml file.
```yaml
mappings:
  - id: France
    tag:
      captures:
        - quality
      concat: '|'
      prefix: ' [ '
      suffix: ' ]'
    match_as_ascii: true
    templates:
      - name: delimiter
        value: '[\s_-]*'
      - name: quality
        value: '(?i)(?P<quality>HD|LQ|4K|UHD)?'
    mapper:
      - tvg_name: TF1 $quality
        tvg_names:
          - '^\s*(FR)?[: |]?TF1!delimiter!!quality!\s*$'
        tvg_id: TF1.fr
        tvg_chno: "1"
        tvg_logo: https://emojipedia-us.s3.amazonaws.com/source/skype/289/shrimp_1f990.png
        group_title:
          - FR
          - TNT
      - tvg_name: TF1 Séries Films $quality
        tvg_names:
          - '^.*TF1!delimiter!Series?!delimiter!Films?(!delimiter!!quality!)\s*$'
        tvg_id: TF1SeriesFilms.fr
        tvg_chno: "20"
        tvg_logo: https://emojipedia-us.s3.dualstack.us-west-1.amazonaws.com/thumbs/120/google/350/shrimp_1f990.png
        group_title:
          - FR
          - TNT
      - tvg_name: TF1 +1 - $quality
        tvg_names:
          - '^.*TF1!delimiter!Series?!delimiter!Films?!delimiter!(\+|plus)1(!delimiter!!quality!)\s*$'
        tvg_id: TF1Plus1.fr
        tvg_chno: "1"
        tvg_logo: https://emojipedia-us.s3.amazonaws.com/source/skype/289/shrimp_1f990.png
        group_title:
          - FR
          - TNT
          - PLUS1
```

## 3. Compilation

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

## 4. The EXTM3U format is an extension of the M3U format.
m3u has become almost a standard for the formation of playlists of media players and media devices.

A file in the EXTM3U format is a text file with the extension m3u or m3u8.

An example of the contents of the file in the EXTM3U format
```
#EXTM3U
#EXTINF:-1 tvg-name="Channel 1" tvg-logo="http://site.domain/channel1_logo.png" group-title="Group 1",Channel 1
http://site.domain/channel1
#EXTINF:-1 tvg-name="Channel 2" tvg-logo="http://site.domain/channel2_logo.png"  group-title="Group 2",Channel 2
http://site.domain/channel2
#EXTINF:-1 tvg-name="Channel 3" tvg-logo="http://site.domain/channel3_logo.png"  group-title="Group 2",Channel 3
http://site.domain/channel3
```
