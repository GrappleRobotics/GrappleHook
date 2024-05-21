import { useEffect, useState } from "react"
import { DeviceInfo, FlexiCanRequest, FlexiCanResponse, FlexiCanStatus } from "../schema"
import { useToasts } from "../toasts"
import { rpc } from "../rpc"
import { Button, Col, ProgressBar, Row } from "react-bootstrap"
import { GrappleDeviceHeaderComponent } from "./Device"

export type FlexiCanProps = {
  info: DeviceInfo,
  invoke: (msg: FlexiCanRequest) => Promise<FlexiCanResponse>
}

export default function FlexiCanComponent(props: FlexiCanProps) {
  const { info, invoke } = props;
  const { addError } = useToasts();

  const [ status, setStatus ] = useState<FlexiCanStatus>();

  useEffect(() => {
    const interval = setInterval(() => {
      rpc<FlexiCanRequest, FlexiCanResponse, "status">(invoke, "status", {})
        .then(setStatus)
        .catch(e => {});  // Discard, it's usually a message to say that the device is disconnected and the UI fragment just hasn't been evicted yet.
    }, 50);
    return () => clearInterval(interval);
  }, []);

  return <div className="powerful-panda">
    <Row className="mb-2">
      <Col>
        <GrappleDeviceHeaderComponent
          info={info}
          invoke={async (msg) => await rpc<FlexiCanRequest, FlexiCanResponse, "grapple">(invoke, "grapple", { msg })}
          start_dfu={async () => await rpc<FlexiCanRequest, FlexiCanResponse, "start_field_upgrade">(invoke, "start_field_upgrade", {})}
        />
      </Col>
    </Row>
    <Row className="mb-2">
      <Col>
        
      </Col>
    </Row>
  </div>
}