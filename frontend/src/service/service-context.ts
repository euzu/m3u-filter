import ConfigService from "./config-service";
import PlaylistService from "./playlist-service";
import FileService from "./file-service";
import AuthService from "./auth-service";

export interface Services {
    config(): ConfigService;

    playlist(): PlaylistService;

    file(): FileService;

    auth(): AuthService;
}

class ServiceContextImpl implements Services {

    private readonly _configService: ConfigService = new ConfigService();
    private readonly _playlistService: PlaylistService = new PlaylistService();
    private readonly _fileService: FileService = new FileService();
    private readonly _authService: AuthService = new AuthService();

    config() {
        return this._configService;
    }

    playlist() {
        return this._playlistService;
    }

    file() {
        return this._fileService;
    }

    auth() {
        return this._authService;
    }
}

const ServiceContext = new ServiceContextImpl();
export default ServiceContext;

