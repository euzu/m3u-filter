import ServerConfig from "../model/server-config";

const getTargetNames = (config: ServerConfig): string[] => {
    return config?.sources.flatMap(s => s.targets)
        .map(t => t.name).filter(n => "default" !== n) || [];
}


const ConfigUtils = {
    getTargetNames
}

export default ConfigUtils;