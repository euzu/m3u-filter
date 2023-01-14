# m3u-filter

m3u-filter is a simple application which can filter entries out of a playlist in EXTM3U format.
If you have a playlist which contains unwanted entries, you can create filter which include or discard entries
based on the header information of the playlist entries.
Currently filter and rename operations support group, name and title fields.

You can run m3u-filter as command line application to update your playlists (manually or as cron job), or you can 
run it in server mode and open the web-ui to see the contents of the playlist, filter/search content and save the filtered groups as a new playlist.

You can define multiple targets for filtering if you want to create multiple playlists from a big playlist.

The config.yml file contains the filter and rename definitions. It should be located next to the exe file or in the current working directory.
You can override this behaviour with the -c argument.
The input file can be defined inside the config.yml file or can be given as -i argument.
If given as argument, it overrides the config file entry.

There are two types of targets ```m3u``` and ```strm```. This can be set by the ```output``` attribute to ```Strm``` or ```M3u```. 
If the attribute is not specified ```M3u``` is created by default.

```Strm``` output has additional options ```underscore_whitespace```, ```cleanup``` and ```kodi_style```.
```underscore_whitespace``` replaces all whitespaces with ```_``` in the path.
```cleanup``` deletes the directory given at ```filename```.
```kodi_style``` tries to rename ```filename``` with [kodi style](https://kodi.wiki/view/Naming_video_files/TV_shows).

The processing order (Filter, Rename and Map) can be configured for each target with:
`processing_order: Frm` (valid values are: Frm, Fmr, Rfm, Rmf, Mfr, Mrf. default is Frm)

The mapping is optional and can be configured for each target with:
`mapping: <list of mapping id's>`
The mappings are defined in a file `mapping.yml`. The filename can be given as `-m` argument.

the config.yml looks like:
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
    processing_order: Frm
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
The input *url* can be an url or filename. If you have a local file you can simply write the name of the file as url:
```yaml
input:
  url: playlist.m3u
```
The input *persist* configuration is for storing the input content. The {} is replaced by a date time tag. If you don't
want to persist the input content, then let it empty.

The filter can have UnaryExpression ```NOT```, BinaryExpression ```AND OR```, and Comparison ```(Group|Title|Name|Url) ~ "regexp"```
Filter fields are Group, Title, Name and Url.

Example filter:  ```((Group ~ "^DE.*") AND (NOT Title ~ ".*Shopping.*")) OR (Group ~ "^AU.*")```

The regular expression syntax is similar to Perl-style regular expressions,
but lacks a few features like look around and backreferences.

The rename supports capture groups. Each group can be adressed with $1, $2 .. in the new_name attribute.
This is needed for players which do not observe the order and sort themselves. In the above example each entry starting
with DE will be prefixed with "1.". 
If you dont care about sorting, you dont need the rename block.

example mapping.yml file.
(If you have some non standard ascii letters like `é`, you can set `match_as_ascii: true`.) 
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

* `mappings.tag` is a struct
    - captures: List of captured variable names like `quality`. The names should be equal to the regexp capture names.
    - concat: if you have more than one captures defined this is the join string between them
    - suffix: suffix for thge tag
    - prefix: prefix for the tag

## The EXTM3U format is an extension of the M3U format.
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

## Starting in server mode for Web-UI
If you want to see the contents of a playlist, you can simply start with the -s (--server)
argument. Other arguments are ignored. A server is started. You can open a browser to view the Web-UI.
According to your configuration, use the printed url on console.
The UI allows you to download a list. You can download the list with the Save Button.
The downloaded list only contains *non-selected* entries.

## Cross compile for windows on linux
If you want to compile this project on linux for windows, you need to do the following steps.

### Install mingw packages for your distribution
For ubuntu type:
```shell
sudo apt-get install gcc-mingw-w64
```
### Install mingw support for rust
```shell
rustup target add x86_64-pc-windows-gnu
rustup toolchain install stable-x86_64-pc-windows-gnu
```

Compile it with:
```sh
cargo build --release --target x86_64-pc-windows-gnu
```
