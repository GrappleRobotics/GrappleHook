import React from "react";
import { Alert, Button, Col, Row, Tab, Tabs } from "react-bootstrap";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faBinoculars, faTriangleExclamation } from "@fortawesome/free-solid-svg-icons";
// import CANINT from "./CANINT";
import { ProviderInfo, WrappedDeviceProviderRequest, WrappedDeviceProviderResponse } from "../schema";
import { useToasts } from "../toasts";
import { rpc } from "../rpc";

type ProviderProps = {
  info: ProviderInfo,
  invoke: (msg: WrappedDeviceProviderRequest) => Promise<WrappedDeviceProviderResponse>
}

export default function ProviderComponent(props: ProviderProps) {
  const { info, invoke } = props;
  const { addError } = useToasts();

  return <React.Fragment>
    <Row className="mb-1">
      <Col>
        <h3> { info.address } <span className="text-muted">({ info.description })</span> </h3>
        <h5 className={info.connected ? "text-success" : "text-danger"}> { info.connected ? "CONNECTED" : "DISCONNECTED" } </h5>
      </Col>
      <Col md="auto">
        <Button variant={info.connected ? "danger" : "success"} onClick={() => {
          if (info.connected)
            rpc<WrappedDeviceProviderRequest, WrappedDeviceProviderResponse, "disconnect">(invoke, "disconnect", {})
              .catch(addError)
          else
            rpc<WrappedDeviceProviderRequest, WrappedDeviceProviderResponse, "connect">(invoke, "connect", {})
              .catch(addError)
        }}>
          { info.connected ? "Disconnect" : "Connect" }
        </Button>
      </Col>
    </Row>
    <Row>
      <Col>
        {
          !info.connected && <Alert variant="warning">
            <h3><FontAwesomeIcon icon={faTriangleExclamation} /> &nbsp; You're not Connected!</h3>
            <p> To get started, hit the <span className="text-success">Connect</span> button above! </p>
          </Alert>
        }
      </Col>
    </Row>
  </React.Fragment>
}
