const dev = {
    app: {
        version: process.env.REACT_APP_VERSION,
    },
};

const prod = {
    app: {
        version: process.env.REACT_APP_VERSION,
    },
};

const config = process.env.REACT_APP_STAGE === 'production' ? prod : dev;

const DefaultConfig = {
    // Add common config values here
    max_attachment_size: 5000000,
    ...config
};

export default DefaultConfig;
