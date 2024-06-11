import React, { useEffect, useState } from "react"
import { DeviceInfo, MitocandriaRequest, MitocandriaResponse, MitocandriaStatus } from "../schema"
import { useToasts } from "../toasts"
import { rpc } from "../rpc"
import { Button, Col, ProgressBar, Row } from "react-bootstrap"
import { GrappleDeviceHeaderComponent } from "./Device"
import "./Mitocandria.scss";
import { confirmModal } from "../Confirm"
import BufferedFormControl from "../BufferedFormControl"
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faCircleInfo, faInfoCircle, faTriangleExclamation } from "@fortawesome/free-solid-svg-icons"

export const CHANNEL_NAMES = [
  "USB1",
  "USB2",
  "5VA",
  "5VB",
  "ADJUSTABLE",
]

export type MitocandriaProps = {
  info: DeviceInfo,
  invoke: (msg: MitocandriaRequest) => Promise<MitocandriaResponse>
}

export default function MitocandriaComponent(props: MitocandriaProps) {
  const { info, invoke } = props;
  const { addError, addWarning } = useToasts();

  const [ status, setStatus ] = useState<MitocandriaStatus>();

  useEffect(() => {
    const interval = setInterval(() => {
      rpc<MitocandriaRequest, MitocandriaResponse, "status">(invoke, "status", {})
        .then(setStatus)
        .catch(e => {});  // Discard, it's usually a message to say that the device is disconnected and the UI fragment just hasn't been evicted yet.
    }, 50);
    return () => clearInterval(interval);
  }, []);

  const changeVoltageSetpoint = async (setpoint: number, channel: number) => {
    let [new_setpoint, new_enabled] = await confirmModal("", {
      data: [String(setpoint), false],
      title: "Set new Voltage Target",
      okText: "Set Voltage",
      renderInner: (n: [string, boolean], onUpdate) => <React.Fragment>
        <p className="text-warning">
          <FontAwesomeIcon icon={faTriangleExclamation} />
          The new voltage will take effect immediately. Make sure you don't overvolt your target device!
        </p>
        <p className="tip">
          <FontAwesomeIcon icon={faCircleInfo} />
          The new voltage must be between 15V and 24V.
        </p>
        <BufferedFormControl enter updateOnDefocus instant value={n[0]} onUpdate={v => onUpdate([ String(v), n[1] ])} />
      </React.Fragment>
    });

    let n = Number(new_setpoint);
    if (Number.isNaN(n)) {
      addError("Could not parse: " + new_setpoint)
    } else if (n < 15.0 || n > 24.0) {
      addError(`Out of Range: ${n.toFixed(2)} must be in the range 15.0 <= V <= 24.0`);
    } else {
      rpc<MitocandriaRequest, MitocandriaResponse, "set_adjustable_channel">(
        invoke,
        "set_adjustable_channel",
        {
          channel: {
            channel: channel,
            voltage: n * 1000.0
          }
        }
      ).catch(addError)
    }
  }

  return <div className="powerful-panda">
    <Row className="mb-2">
      <Col>
        <GrappleDeviceHeaderComponent
          info={info}
          invoke={async (msg) => await rpc<MitocandriaRequest, MitocandriaResponse, "grapple">(invoke, "grapple", { msg })}
          start_dfu={async () => await rpc<MitocandriaRequest, MitocandriaResponse, "start_field_upgrade">(invoke, "start_field_upgrade", {})}
        />
      </Col>
    </Row>
    <Row className="mb-2">
      <Col>
        {
          status?.last_update?.channels?.map((channel, i) => {
            const current = channel.data.current / 1000.0;
            return <Row className="mb-2">
              <Col>
                <h4>
                  { CHANNEL_NAMES[i] } &nbsp;
                  {
                    (channel.type == "Switchable" || channel.type == "Adjustable") &&
                      <Button size="sm" variant={channel.data.enabled ? "green" : "red"} onClick={() => rpc<MitocandriaRequest, MitocandriaResponse, "set_switchable_channel">(invoke, "set_switchable_channel", {
                        channel: {
                          channel: i,
                          enabled: !channel.data.enabled,
                        }
                      }).catch(addError)}>
                        { channel.data.enabled ? "ENABLED" : "DISABLED" }
                      </Button>
                    || <span className="text-muted"><i>Non-Switchable</i></span>
                  } &nbsp;
                  {
                    channel.type == "Adjustable" &&
                      <Button size="sm" variant="purple" onClick={() => changeVoltageSetpoint(channel.data.voltage_setpoint / 1000.0, i)}>
                        { (channel.data.voltage / 1000.0).toFixed(2) }V
                      </Button>
                  }
                </h4>
                <Row>
                  <Col>
                    <ProgressBar
                      className="powerful-panda-bar"
                      variant={current < 3.0 ? "green" : current < 5.0 ? "orange" : "red" }
                      now={Math.max((current / 10.0 * 100.0), 0.0)}
                    />
                  </Col>
                  <Col md={2}>
                    <span className="text-muted">{ current.toFixed(2) }A</span>
                  </Col>
                </Row>
              </Col>
            </Row>
          })
        }
      </Col>
    </Row>
  </div>
}