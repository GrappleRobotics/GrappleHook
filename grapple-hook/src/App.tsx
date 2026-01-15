import { invoke } from "@tauri-apps/api/core";
import { listen, Event, EventCallback } from "@tauri-apps/api/event";
import React, { useEffect } from "react";
import { Alert, Button, Col, Form, InputGroup, Nav, Row, Tab, Toast, ToastContainer } from "react-bootstrap";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faInfo, faInfoCircle, faTriangleExclamation, faUpload } from "@fortawesome/free-solid-svg-icons";
import Bug from "./Bug";
import ProviderManagerComponent from "./providers/ProviderManager";
import { LightReleaseResponse, ProviderManagerRequest, ProviderManagerResponse } from "./schema";
import ToastProvider, { useToasts } from "./toasts";
import DebugContextProvider, { useDebugCtx } from "./DebugContext";

export default class App extends React.Component<{}> {
  render() {
    return <ToastProvider>
      <DebugContextProvider>
        <AppInner />
      </DebugContextProvider>
    </ToastProvider>
  }
}
const our_invoke = async (msg: ProviderManagerRequest): Promise<ProviderManagerResponse> => {
  try {
    let result = await invoke("provider_manager_rpc", { msg: msg });
    return result as ProviderManagerResponse;
  } catch (e) {
    throw new Error(e as string)
  }
}

export function AppInner() {
  const { toasts, addInfo, removeToast } = useToasts();
  const { mode: debugMode } = useDebugCtx();
  
  useEffect(() => {
    const interval = setTimeout(() => {
      invoke("is_update_available").then((update) => {
        const release = update as LightReleaseResponse;
        addInfo(<span>
          Please update to <a target="_blank" href={release.html_url}>{ release.tag_name }</a>
        </span>, "Update Available");
      }).catch(e => {});
    }, 1000);
    return () => clearTimeout(interval)
  }, []);

  return <div className="container">
    <img src="icon.png" height={30} style={{ marginRight: "20px" }} />
    <i style={{fontSize: "1.5em", color: debugMode == "debug" ? "gold" : "white"}}>Grapple<strong>Hook</strong></i>
    <hr />
    
    <ProviderManagerComponent invoke={our_invoke} />
    
    <ToastContainer className="m-3" position="bottom-end">
      {
        toasts.map((t, i) => <Toast key={i} bg={t.variant} onClose={() => removeToast(i)}>
          <Toast.Header> { t.title } <span className="me-auto"></span> </Toast.Header>
          <Toast.Body> { t.message } </Toast.Body>
        </Toast>)
      }
    </ToastContainer>
  </div>
}
