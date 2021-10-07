import ConfigService from "./config-service";
import PlaylistService from "./playlist-service";
import FileService from "./file-service";

export interface Services {
    config(): ConfigService;

    playlist(): PlaylistService;

    file(): FileService;
}

class ServiceContextImpl implements Services {

    private readonly _configService: ConfigService = new ConfigService();
    private readonly _playlistService: PlaylistService = new PlaylistService();
    private readonly _fileService: FileService = new FileService();

    config() {
        return this._configService;
    }

    playlist() {
        return this._playlistService;
    }

    file() {
        return this._fileService;
    }
}

const ServiceContext = new ServiceContextImpl();
export default ServiceContext;

