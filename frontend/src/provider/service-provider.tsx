import React, {useContext} from "react";
import ServiceContext, {Services} from "../service/service-context";

const ServiceCtx = React.createContext(ServiceContext);
const ServiceCtxProvider = ServiceCtx.Provider;

export function ServiceProvider(props: any) {
    return <ServiceCtxProvider value={ServiceContext}>{props.children}</ServiceCtxProvider>
}

export function useServices(): Services  {
    return useContext(ServiceCtx);
}