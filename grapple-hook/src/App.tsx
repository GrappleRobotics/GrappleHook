import { invoke } from "@tauri-apps/api/tauri";
import { listen, Event } from "@tauri-apps/api/event";
import React from "react";
import { Alert, Button, Col, Form, InputGroup, Nav, Row, Tab, Toast, ToastContainer } from "react-bootstrap";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faInfo, faInfoCircle, faTriangleExclamation, faUpload } from "@fortawesome/free-solid-svg-icons";
import Bug from "./Bug";
import ProviderManagerComponent from "./providers/ProviderManager";
import { ProviderManagerRequest, ProviderManagerResponse } from "./schema";
import ToastProvider, { useToasts } from "./toasts";

export default class App extends React.Component<{}> {
  render() {
    return <ToastProvider>
      <AppInner />
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
  const { toasts, removeToast } = useToasts();

  return <div className="container">
    <img src="icon.png" height={30} style={{ marginRight: "20px" }} />
    <i style={{fontSize: "1.5em"}}>Grapple<strong>Hook</strong></i>
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
