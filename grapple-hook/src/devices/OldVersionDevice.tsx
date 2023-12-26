import { useEffect, useState } from "react";
import { DeviceInfo, OldVersionDeviceRequest, OldVersionDeviceResponse } from "../schema"
import { useToasts } from "../toasts";
import { rpc } from "../rpc";
import { Alert, Col, Row } from "react-bootstrap";
import { FirmwareUpdateComponent, GrappleDeviceHeaderComponent } from "./Device";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faTriangleExclamation } from "@fortawesome/free-solid-svg-icons";

export type OldVersionDeviceComponentProps = {
  info: DeviceInfo,
  invoke: (msg: OldVersionDeviceRequest) => Promise<OldVersionDeviceResponse>
}

export default function OldVersionDevice(props: OldVersionDeviceComponentProps) {
  const { info, invoke } = props;
  const { addError } = useToasts();

  const [ versionError, setVersionError ] = useState<string>("Invalid Version");
  const [ firmwareUrl, setFirmwareUrl ] = useState<string | null>(null);

  useEffect(() => {
    const timeout = setTimeout(() => {
      rpc<OldVersionDeviceRequest, OldVersionDeviceResponse, "get_error">(invoke, "get_error", {})
        .then(setVersionError)
        .catch(addError);
      
      rpc<OldVersionDeviceRequest, OldVersionDeviceResponse, "get_firmware_url">(invoke, "get_firmware_url", {})
        .then(setFirmwareUrl)
        .catch(addError);
    }, 100);
    return () => clearTimeout(timeout);
  }, []);

  return <div>
    <Row className="mb-2">
      <Col>
        <GrappleDeviceHeaderComponent
          info={info}
          invoke={async (msg) => await rpc<OldVersionDeviceRequest, OldVersionDeviceResponse, "grapple">(invoke, "grapple", { msg })}
          start_dfu={async () => await rpc<OldVersionDeviceRequest, OldVersionDeviceResponse, "start_field_upgrade">(invoke, "start_field_upgrade", {})}
        />
      </Col>
    </Row>
    <Row className="mb-2">
      <Alert variant="danger">
        <FontAwesomeIcon icon={faTriangleExclamation} size="2x" /> &nbsp;
        <span style={{ fontSize: '2em' }}> This device is out of date! </span>
        <br />
        <span> You have to update this device before you can configure it in GrappleHook. </span>
        <br />
        <span>Error: <strong>{ versionError }</strong></span>
        <br />
        <strong> Click <span className="text-purple">"Firmware Update"</span> and upload a new firmware version! </strong>
        <br />
        {
          firmwareUrl && <span> Download Firmware Here: <a href={firmwareUrl} style={{ color: "blue" }} target="_blank">{ firmwareUrl }</a> </span>
        }
      </Alert>
    </Row>
  </div>
}