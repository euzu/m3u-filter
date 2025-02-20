import ConfigService from "./config-service";
import PlaylistService from "./playlist-service";
import FileService from "./file-service";
import AuthService from "./auth-service";
import UserConfigService from "./user-config-service";

export interface Services {
    config(): ConfigService;

    playlist(): PlaylistService;

    file(): FileService;

    auth(): AuthService;

    userConfig(): UserConfigService;
}

class ServiceContextImpl implements Services {

    private readonly _configService: ConfigService = new ConfigService();
    private readonly _playlistService: PlaylistService = new PlaylistService();
    private readonly _fileService: FileService = new FileService();
    private readonly _authService: AuthService = new AuthService();
    private readonly _userConfigService: UserConfigService = new UserConfigService();

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

    userConfig(): UserConfigService {
        return this._userConfigService;
    }
}

const ServiceContext = new ServiceContextImpl();
export default ServiceContext;

