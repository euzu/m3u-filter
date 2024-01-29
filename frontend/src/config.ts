const location = window.location;

const dev = {
    app: {
        version: process.env.REACT_APP_VERSION,
    },
    api: {
        serverUrl: 'http://10.41.41.41:8901/api/v1/',
    }
};

const prod = {
    app: {
        version: process.env.REACT_APP_VERSION,
    },
    api: {
        serverUrl: location.origin + '/api/v1/',
    },
};

const config = process.env.REACT_APP_STAGE === 'production' ? prod : dev;

const DefaultConfig = {
    // Add common config values here
    max_attachment_size: 5000000,
    ...config
};

export default DefaultConfig;
