import { faCheck, faInfoCircle, faPencil, faShuffle, faTriangleExclamation } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { invoke } from "@tauri-apps/api";
import React, { useEffect, useState } from "react";
import { Button, Col, FormControl, FormSelect, ProgressBar, Row } from "react-bootstrap";
import SimpleTooltip from "../SimpleTooltip";
import { confirmModal } from "../Confirm";
import "./LaserCan.scss";
import { DeviceInfo, LaserCanRequest, LaserCanResponse, LaserCanStatus, LaserCanTimingBudget, LightReleaseResponse } from "../schema";
import { useToasts } from "../toasts";
import { rpc } from "../rpc";
import { FirmwareUpdateComponent, GrappleDeviceHeaderComponent } from "./Device";

export type LaserCanRoi = {
  x: number, y: number,
  w: number, h: number,
}

function isRoiValid(roi: LaserCanRoi) {
  return (roi.x % 1 == 0) && (roi.y % 1 == 0) && (roi.w % 2 == 0) && (roi.h % 2 == 0) && (roi.w >= 4) && (roi.h >= 4);
}

export type LaserCanState = {
  device_id: number,
  ambient: number,
  distance: number,
  status: number,
  ranging_long: boolean,
  budget_ms: number,
  roi: LaserCanRoi
};

export type LaserCanComponentProps = {
  info: DeviceInfo,
  invoke: (msg: LaserCanRequest) => Promise<LaserCanResponse>
}

export default function LaserCanComponent(props: LaserCanComponentProps) {
  const { info, invoke } = props;
  const { addError } = useToasts();

  const [ status, setStatus ] = useState<LaserCanStatus>();
  const [ updateDetails, setUpdateDetails ] = useState<LightReleaseResponse | null>(null);

  useEffect(() => {
    const interval = setInterval(() => {
      rpc<LaserCanRequest, LaserCanResponse, "status">(invoke, "status", {})
        .then(setStatus)
        .catch(e => {});  // Discard, it's usually a message to say that the device is disconnected and the UI fragment just hasn't been evicted yet.
    }, 50);
    
    rpc<LaserCanRequest, LaserCanResponse, "check_for_new_firmware">(invoke, "check_for_new_firmware", {})
      .then(setUpdateDetails)
      .catch(e => {})
    
    return () => clearInterval(interval);
  }, []);

  const toggleRange = async () => {
    if (status?.last_update?.mode === "Long")
      rpc<LaserCanRequest, LaserCanResponse, "set_range">(invoke, "set_range", { mode: "Short" }).catch(addError)
    else
      rpc<LaserCanRequest, LaserCanResponse, "set_range">(invoke, "set_range", { mode: "Long" }).catch(addError)
  }
  
  const changeTimingBudget = async () => {
    let new_budget = await confirmModal("", {
      data: String(status?.last_update!.budget),
      title: "Set New Timing Budget",
      okText: "Set Timing Budget",
      renderInner: (n: string, onUpdate) => <React.Fragment>
        <p> Set New Timing Budget (ms) </p>
        <p className="tip"> <FontAwesomeIcon icon={faInfoCircle} /> The Timing Budget is how long each measurement is taken for. Smaller values
        will give you faster results, but will be less accurate. </p>
        <FormSelect value={n} onChange={e => onUpdate(e.target.value)}>
          <option value="TB20ms">TB20ms</option>
          <option value="TB33ms">TB33ms</option>
          <option value="TB50ms">TB50ms</option>
          <option value="TB100ms">TB100ms</option>
        </FormSelect>
      </React.Fragment>
    });

    rpc<LaserCanRequest, LaserCanResponse, "set_timing_budget">(invoke, "set_timing_budget", { budget: new_budget as LaserCanTimingBudget }).catch(addError)
  }

  const changeROI = async () => {
    let new_roi = undefined;

    while (new_roi == undefined || !isRoiValid(new_roi)) {
      new_roi = await confirmModal("", {
        data: status?.last_update?.roi,
        title: "Set New Region of Interest",
        okText: "Set ROI",
        renderInner: (n: LaserCanRoi, onUpdate) => <React.Fragment>
          <p className="tip"> <FontAwesomeIcon icon={faInfoCircle} /> The LaserCAN sensor is made of 16x16 small sensor elements (SPADs). Selecting a Region
          of interest allows you to define which SPADs operate, changing the field of view of the sensor and the total amount of accumulated light. Smaller
          Regions of Interest (ROIs) provide narrower field of view and better ambient noise rejection, but may limit your range. </p>
          <hr />
          <LaserCanRoiPicker roi={n} onUpdate={onUpdate} />
          <hr />
          { isRoiValid(n) && 
            <span className="text-success">
              <FontAwesomeIcon icon={faCheck} />
            </span>
            || <span className="text-warning">
              <SimpleTooltip id="roi-tip" tip="A valid ROI must be centered on a whole number, with heights and widths divisible by 2. The minimum size is 4x4."> <FontAwesomeIcon icon={faTriangleExclamation} /> </SimpleTooltip>
            </span>
          } &nbsp;
          <span className="tip">Selected ROI: </span> { n.w }x{ n.h } @ ({ n.x }, { n.y })
        </React.Fragment>
      })
    }

    rpc<LaserCanRequest, LaserCanResponse, "set_roi">(invoke, "set_roi", { roi: new_roi }).catch(addError)
  }

  if (status === undefined || status.last_update == null)
    return <div />;
  
  const { roi, mode, distance_mm, budget, ambient } = status.last_update;

  return <div className="lasercan">
    <Row className="mb-2">
      <Col>
        <GrappleDeviceHeaderComponent
          info={info}
          invoke={async (msg) => await rpc<LaserCanRequest, LaserCanResponse, "grapple">(invoke, "grapple", { msg })}
          start_dfu={async () => await rpc<LaserCanRequest, LaserCanResponse, "start_field_upgrade">(invoke, "start_field_upgrade", {})}
          update_details={updateDetails}
        />
      </Col>
    </Row>
    <Row className="mb-2">
      <Col md={3} className="device-field-label"> Ranging Mode </Col>
      <Col md={3}>
        { mode.toUpperCase() }
        { mode === "Long" && <span className="text-warning"><SimpleTooltip id="long-range-tip" tip="Long Range is more susceptible to ambient light noise"> <FontAwesomeIcon icon={faTriangleExclamation} /> </SimpleTooltip></span> } &nbsp;
        <Button size="sm" onClick={() => toggleRange()}> <FontAwesomeIcon icon={faShuffle} /> </Button>
      </Col>
      <Col md={3} className="device-field-label"> Timing Budget </Col>
      <Col md={3}>
        { budget } &nbsp;
        <Button size="sm" onClick={() => changeTimingBudget()}> <FontAwesomeIcon icon={faPencil} /> </Button>
      </Col>
    </Row>
    <Row className="mb-3">
      <Col md={3} className="device-field-label">Region of Interest</Col>
      <Col md={4}>
        {roi.w}x{roi.h} @ ({roi.x}, {roi.y}) &nbsp;
        <Button size="sm" onClick={() => changeROI()}> <FontAwesomeIcon icon={faPencil} /> </Button>
      </Col>
    </Row>
    <Row>
      <Col md="auto" className="device-field-label"> Distance </Col>
      <Col>
        <ProgressBar
          className="lasercan-bar"
          variant={status.last_update!.status == 0 ? "success" : "danger"} 
          now={status.last_update!.status != 0 ? 100 : distance_mm / 4000 * 100}
          label={status.last_update!.status == 0 ? "" : "Out of Range"}
        />
      </Col>
      <Col md={2}>
        {
          status.last_update!.status == 0 ? `${distance_mm / 1000}m` : "---"
        }
      </Col>
    </Row>
    <Row>
    <Col md="auto" className="device-field-label"> Ambient </Col>
      <Col>
        <ProgressBar
          className="lasercan-bar"
          variant={ambient > 2000 ? "danger" : "warning"}
          now={ambient / 2000 * 100}
        />
      </Col>
      <Col md={2}>
        {
          ambient
        }
      </Col>
    </Row>
  </div>
}

type LaserCanRoiPickerProps = {
  roi: LaserCanRoi,
  onUpdate: (roi: LaserCanRoi) => void
};

type LaserCanRoiPickerState = {
  active: boolean,
  activeCorner: [number, number]
};

export class LaserCanRoiPicker extends React.Component<LaserCanRoiPickerProps, LaserCanRoiPickerState> {
  readonly state: LaserCanRoiPickerState = { active: false, activeCorner: [0, 0] };

  select = (x: number, y: number) => {
    if (this.state.active) {
      this.setState({ active: false });
    } else {
      this.props.onUpdate({ x: x, y: y, w: 0, h: 0 });
      this.setState({ active: true, activeCorner: [x, y] });
    }
  }

  hover = (x: number, y: number) => {
    if (this.state.active) {
      let center = [(this.state.activeCorner[0] + x) / 2 + 0.5, (this.state.activeCorner[1] + y) / 2 + 0.5];
      let width = Math.abs(this.state.activeCorner[0] - x) + 1;
      let height = Math.abs(this.state.activeCorner[1] - y) + 1;

      this.props.onUpdate({ x: center[0], y: center[1], w: width, h: height });
    }
  }

  render() {
    const valid = isRoiValid(this.props.roi);
    return <div className="lasercan-roi-picker">
      {
        [...Array(16).keys()].map(row => (
          [...Array(16).keys()].map(column => (
            <div
              className="lasercan-roi-picker-element"
              data-selected={
                row >= (this.props.roi.y - this.props.roi.h / 2) && row < (this.props.roi.y + this.props.roi.h / 2)
                && column >= (this.props.roi.x - this.props.roi.w / 2) && column < (this.props.roi.x + this.props.roi.w / 2)
              }
              data-valid={valid}
              style={{ left: `${column * 1/16 * 100}%`, top: `${row * 1/16 * 100}%` }}
              onClick={() => this.select(column, row)}
              onMouseEnter={() => this.hover(column, row)}
            />
            ))
          ))
        }
        <div className="lasercan-roi-picker-center-x" />
        <div className="lasercan-roi-picker-center-y" />
    </div>
  }
}