import { useEffect, useRef, useState } from "react";
import { CanLogRequest, CanLogResponse, Filter as CANFilter, MailboxItem, GrappleDeviceMessage } from "../schema"
import { Button, Col, Form, FormControl, Row, Table } from "react-bootstrap";
import { rpc } from "../rpc";
import { useToasts } from "../toasts";

import { stringify as csv_stringify } from "csv-stringify/browser/esm/sync";
import { parse as csv_parse } from "csv-parse/browser/esm/sync";

import { confirmModal } from "../Confirm";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faFastBackward, faFastForward, faMagnifyingGlass, faPause, faPlay } from "@fortawesome/free-solid-svg-icons";

export type CanLogProps = {
  invoke: (msg: CanLogRequest) => Promise<CanLogResponse>,
  enabled: boolean
}

const DEVICE_TYPES: { [mid: number]: string } = {
  0: "<bcast>",
  1: "Robot Ctrlr",
  2: "Motor Ctrlr",
  3: "Relay Ctrlr",
  4: "Gyro",
  5: "Accelerometer",
  6: "Dist Sensor",
  7: "Encoder",
  8: "Power Dist.",
  9: "Pneumatics Ctrlr",

  11: "IO B/out",
  12: "Servo Ctrlr",
  13: "Colour Sensor",

  31: "Firmware Update",
};

const MANUFACTURERS: { [mid: number]: string } = {
  0: "<bcast>",
  1: "NI",
  2: "Lum. Micro",
  3: "DEKA",
  4: "CTRE",
  5: "REV",
  6: "Grapple",
  7: "MindSens.",
  8: "<team>",
  9: "Kauai Labs",
  10: "Copperforge",
  11: "PWF",
  12: "Studica",
  13: "TTB",
  14: "Redux",
  15: "AndyMark",
  16: "Vivid Host.",
  17: "Vertos",
  18: "SWYFT",
  19: "Lumyn",
  20: "Brushland",
};

function idAnnotation(id: number, map: { [id: number]: string }) {
  return id in map &&
    <div className="text-muted" style={{ display: "inline", marginLeft: "0.5em", fontSize: "0.75em" }}>
      <em>{ map[id] }</em>
    </div>
}

interface ReplayFrame {
  time_ms: number,
  id_type: number,
  id_manufacturer: number,
  id_api_class: number,
  id_api_index: number,
  id_device_id: number,
  data: number[]
}

function fromHex(n?: string) {
  return n ? parseInt(n, 16) : undefined;
}

function decodeReplay(rows: any[]): ReplayFrame[] {  
  const unordered_frames = rows.map(row => {
    const time = parseFloat(row["time_raw"] ?? row["time"]);
    const id_type = fromHex(row["id_type_hex"]) ?? parseInt(row["id_type"]);
    const id_manufacturer = fromHex(row["id_manufacturer_hex"]) ?? parseInt(row["id_manufacturer"]);
    const id_api_class = fromHex(row["id_api_class_hex"]) ?? parseInt(row["id_api_class"]);
    const id_api_index = fromHex(row["id_api_index_hex"]) ?? parseInt(row["id_api_index"]);
    const id_device_id = parseInt(row["id_device_id"]);
    const data: number[] = ((row["data"] ?? row["data_hex"])?.match(/[0-9a-fA-F]{2}/gi) ?? []).map((t: string) => parseInt(t, 16));

    const validate_numbers: [number | undefined, string][] = [
      [time, "time"],
      [id_type, "id_type"],
      [id_manufacturer, "id_manufacturer"],
      [id_api_class, "id_api_class"],
      [id_api_index, "id_api_index"],
      [id_device_id, "id_device_id"],
    ];

    for (const [v, name] of validate_numbers) {
      if (v == null || v == undefined || Number.isNaN(v)) {
        throw new Error(`${name} is invalid / could not be parsed as a number (value: ${v}, row: ${JSON.stringify(row)})`);
      }
    }

    if (data == null || data == undefined) {
      throw new Error(`Data could not be parsed (row: ${JSON.stringify(row)})`);
    }

    for (const item of data) {
      if (item == null || item == undefined || Number.isNaN(item)) {
        throw new Error(`Data element could not be converted to number (row: ${JSON.stringify(row)})`);
      }
    }

    return {
      time_ms: time,
      id_type: id_type,
      id_manufacturer: id_manufacturer,
      id_api_class: id_api_class,
      id_api_index: id_api_index,
      id_device_id: id_device_id,
      data: data,
    };
  }).filter(v => v);

  // @ts-ignore
  const ordered_frames = unordered_frames.toSorted((a: any, b: any) => a.time_ms - b.time_ms);
  const retimed_frames = ordered_frames.map((a: any) => ({ ...a, time_ms: a.time_ms - ordered_frames[0].time_ms }));

  return retimed_frames;
}

export default function CanLog({ invoke, enabled }: CanLogProps) {
  const { addError } = useToasts();

  const [ filters, setFilters ] = useState<CANFilter[]>([]);
  const latestSeqRef = useRef(0);
  const totalCaptured = useRef(0);
  const [ msgHistory, setMsgHistory ] = useState<MailboxItem[]>([]);

  const [ maxHistory, setMaxHistory ] = useState<number>(4096);
  const [ maxDisplayableHistory, setMaxDisplayableHistory ] = useState<number>(128);
  const [ running, setRunning ] = useState<boolean>(false);
  const [ paused, setPaused ] = useState<boolean>(false);

  const [ replayFile, setReplayFile ] = useState<null | { name: string, frames: ReplayFrame[] }>(null);
  const [ replayRunning, setReplayRunning ] = useState<boolean>(false);
  const [ replayRemaining, setReplayRemaining ] = useState<number>(0);
  const replayIdx = useRef(0);
  const replayTimeout = useRef<NodeJS.Timeout | undefined>();

  useEffect(() => {
    rpc<CanLogRequest, CanLogResponse, "set_log_enabled">(invoke, "set_log_enabled", { enabled: running })
      .catch(addError);
  }, [running]);

  useEffect(() => {
    rpc<CanLogRequest, CanLogResponse, "set_filters">(invoke, "set_filters", { filters: filters })
      .catch(addError);
  }, [filters]);

  useEffect(() => {
    const interval = setInterval(() => {
      if (running && !paused) {
        rpc<CanLogRequest, CanLogResponse, "read_after">(invoke, "read_after", { seq: latestSeqRef.current })
          .then(newItems => {
            if (newItems.length > 0) {
              setMsgHistory((lastHistory) => {
                latestSeqRef.current = newItems[newItems.length - 1].seq;

                let newHistory = [ ...newItems.reverse(), ...lastHistory, ]
                totalCaptured.current += newItems.length;
                if (newHistory.length > maxHistory) {
                  newHistory = newHistory.slice(0, maxHistory);
                }

                return newHistory;
              });
            }
          })
          .catch(addError);
      }
    }, 50);

    return () => clearInterval(interval);
  }, [running, paused]);

  const stepReplay = () => {
    if (replayFile && replayRunning) {
      const currentIdx = replayIdx.current;
      if (currentIdx >= replayFile.frames.length) {
        setReplayRunning(false);
      } else {
        replayIdx.current += 1;
        const frame = replayFile.frames[currentIdx];
  
        rpc<CanLogRequest, CanLogResponse, "send_raw">(invoke, "send_raw", {
          id: {
            device_type: frame.id_type,
            manufacturer: frame.id_manufacturer,
            api_class: frame.id_api_class,
            api_index: frame.id_api_index,
            device_id: frame.id_device_id
          },
          data: frame.data
        }).then(() => {
          const to = replayTimeout.current;
          if (to) {
            clearTimeout(to);
          }
          
          setReplayRemaining(replayFile.frames.length - (currentIdx + 1));

          if (replayFile.frames.length > currentIdx + 1) {
            const dt = replayFile.frames[currentIdx + 1].time_ms - frame.time_ms;
            replayTimeout.current = setTimeout(stepReplay, dt);
          } else {
            setReplayRunning(false);
          }
        }).catch(e => addError(`Could not send CAN frame: ${e}`))
      }
    }
  };

  const rewindReplay = () => {
    const to = replayTimeout.current;
    if (to) {
      clearTimeout(to);
    }

    setReplayRunning(false);

    replayIdx.current = 0;
    if (replayFile) {
      setReplayRemaining(replayFile.frames.length)
    }
  };

  useEffect(() => {
    if (replayRunning && replayFile) {
      stepReplay();
    }

    return () => {
      const to = replayTimeout.current;
      if (to) {
        clearTimeout(to);
      }
    }
  }, [replayRunning, replayFile]);

  useEffect(() => {
    if (replayFile) {
      replayIdx.current = 0;
      setReplayRemaining(replayFile.frames.length);
      setReplayRunning(false);
    } else {
      replayIdx.current = 0;
      setReplayRemaining(0);
      setReplayRunning(false);
    }
  }, [replayFile]);

  const clear = () => {
    rpc<CanLogRequest, CanLogResponse, "clear">(invoke, "clear", {})
      .then(() => setMsgHistory([]))
      .catch(addError)
  }

  const dlFile = (filename: string, type: string, content: string) => {
    const blob = new Blob([content], { type: type });
    const href = URL.createObjectURL(blob);

    const link = document.createElement("a");
    link.href = href;
    link.download = filename;
    document.body.appendChild(link);
    link.click();

    document.body.removeChild(link);
    URL.revokeObjectURL(href);
  }

  const exportToCsv = () => {
    const data = [
      ["time_raw", "id_type_hex", "id_manufacturer_hex", "id_api_class_hex", "id_api_index_hex", "id_device_id", "data_hex", "decoded"],
      ...msgHistory.map(msg => [
        msg.raw.timestamp,
        msg.raw.id.device_type.toString(16).padStart(2, "0"),
        msg.raw.id.manufacturer.toString(16).padStart(2, "0"),
        msg.raw.id.api_class.toString(16).padStart(2, "0"),
        msg.raw.id.api_index.toString(16).padStart(2, "0"),
        msg.raw.id.device_id,
        msg.raw.data.map(x => x.toString(16).padStart(2, "0")).join(" "),
        msg.grpl_defrag ? JSON.stringify(msg.grpl_defrag) : null
      ])
    ];

    dlFile("grplhook-canlog-capture.csv", "text/csv", csv_stringify(data));
  }

  const exportToJson = () => {
    const data = {
      filters: filters,
      packets: msgHistory,
    };

    dlFile("grplhook-canlog-capture.json", "application/json", JSON.stringify(data))
  }

  const loadFromCsv = async () => {
    let f: File | null = await confirmModal("", {
      data: null,
      title: "Load CAN frames from file (CSV)",
      okText: "Load",
      renderInner: (file, setFile) => {
        return <>
          <p>Upload a CSV file with the following format:</p>
          <Table>
            <thead>
              <tr>
                <th>Column</th>
                <th>Format</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td><code>time_raw</code></td>
                <td>
                  Time (in milliseconds).<br />
                  <p className="text-muted">The file will be played in time-order.</p>
                </td>
              </tr>
              <tr>
                <td><code>id_type or id_type_hex</code></td>
                <td>The Device Type portion of the ID (decimal or hex)</td>
              </tr>
              <tr>
                <td><code>id_manufacturer or id_manufacturer_hex</code></td>
                <td>The Manufacturer portion of the ID (decimal or hex)</td>
              </tr>
              <tr>
                <td><code>id_api_class or id_api_class_hex</code></td>
                <td>The API Class portion of the ID (decimal or hex)</td>
              </tr>
              <tr>
                <td><code>id_api_index or id_api_index_hex</code></td>
                <td>The API Index portion of the ID (decimal or hex)</td>
              </tr>
              <tr>
                <td><code>id_device_id</code></td>
                <td>The Device ID portion of the ID</td>
              </tr>
              <tr>
                <td><code>data or data_hex</code></td>
                <td>The CAN Frame Data. Always given in hexadecimal (with or without spaces)</td>
              </tr>
            </tbody>
          </Table>
          <FormControl type="file" accept=".csv" onChange={e => setFile((e.target as any).files[0])} />
        </>
      }
    });

    if (f != null) {
      const reader = new FileReader();
      // @ts-ignore
      const name = f.name;

      reader.addEventListener("loadend", (event) => {
        const data = event.target?.result as string;
        const records = csv_parse(data, {
          columns: true, skip_empty_lines: true
        });

        if (records.length > 0) {
          try {
            const decoded = decodeReplay(records);
            
            setReplayFile({
              frames: decoded,
              name: name
            });
          } catch (e) {
            addError(`Could not load file: ${e}`)
          }
        }
      });
      
      reader.readAsText(f);
    }
  }
  
  const viewDecodeModal = async (data: any) => {
    await confirmModal("", {
      title: "Decoded Packet",
      renderInner: () => <>
        <code>
          <pre style={{ color: "#faf" }}>
            {JSON.stringify(data, null, 2)}
          </pre>
        </code>
      </>
    })
  };

  return <div className="canlog">
    <Row>
      <Col md="auto">
        <h4>CANLog</h4>

        <Row>
          <Col>
            <strong className="text-muted">Filter (incoming):</strong>

            <Form.Check
              name="filt-grpl"
              label="Grapple (Decoded)"
              type="checkbox"
              checked={filters.includes("GrappleOnly")}
              onChange={() => {
                if (filters.includes("GrappleOnly")) {
                  setFilters(filts => filts.filter(f => f != "GrappleOnly"))
                } else {
                  setFilters(filts => [ ...filts, "GrappleOnly" ])
                }
              }}
            />

            {/* TODO: More filters */}
          </Col>
        </Row>
      </Col>

      <Col md="auto">
        <p className="m-0"><strong className="text-muted">Recording Controls</strong></p>

        <Button
          size="sm"
          className="m-1"
          onClick={() => setRunning(!running)}
          variant={running ? "red" : "green"}
        >
          { running ? "STOP" : "RUN" }
        </Button>
        &nbsp;
        <Button
          size="sm"
          className="m-1"
          onClick={() => setPaused(!paused)}
          variant={paused ? "green" : "orange"}
          disabled={!running}
        >
          { paused ? "RESUME" : "FREEZE" }
        </Button>
        &nbsp;
        <Button
          size="sm"
          className="m-1"
          onClick={() => clear()}
          variant="hazard-red"
        >
          CLEAR
        </Button>
      </Col>
      
      <Col md="auto">
        <p className="m-0"><strong className="text-muted">Save to...</strong></p>

        <Button
          className="m-1"
          size="sm"
          onClick={exportToJson}
          variant="primary"
        >
          JSON
        </Button>

        <Button
          className="m-1"
          size="sm"
          onClick={exportToCsv}
          variant="primary"
        >
          CSV
        </Button>
      </Col>

      <Col md="auto">
        <p className="m-0"><strong className="text-muted">Load from...</strong></p>
        <Button
          className="m-1"
          size="sm"
          onClick={loadFromCsv}
          variant="secondary"
        >
          CSV
        </Button>

      </Col>
    </Row>

    {
      replayFile && <>
        <Row>
          <Col>
            <p className="m-0 mt-1 text-muted"><strong>Loaded Replay:</strong> { replayFile.name } ({replayFile.frames.length} frames over {replayFile.frames[replayFile.frames.length - 1].time_ms}ms)</p>

            <Button
              className="m-1"
              size="sm"
              variant="orange"
              onClick={rewindReplay}
              disabled={replayFile == null}
            >
              <FontAwesomeIcon icon={faFastBackward} />
            </Button>

            <Button
              className="m-1"
              size="sm"
              variant={ replayRunning ? "orange" : "green" }
              onClick={() => setReplayRunning(!replayRunning)}
              disabled={replayFile == null || replayRemaining == 0}
            >
              <FontAwesomeIcon icon={replayRunning ? faPause : faPlay} /> &nbsp; ({ replayRemaining } frames)
            </Button>
          </Col>
        </Row>
      </>
    }

    <Row>
      <Col>
        <span className="text-muted">
          { msgHistory.length } frame(s) stored, { totalCaptured.current } captured in total.
        </span>
      </Col>
    </Row>

    <Row>
      <Col>
        <Table size="sm" striped bordered hover className="small">
          <thead>
            <tr>
              <th>Time (raw)</th>
              <th>ID[type]</th>
              <th>ID[manu.]</th>
              <th>ID[acls]</th>
              <th>ID[aidx]</th>
              <th>ID[id]</th>
              <th>Data (hex)</th>
              <th>Decoded</th>
            </tr>
          </thead>
          <tbody>
            {
              msgHistory.slice(0, maxDisplayableHistory).map(msg => [
                <tr key={msg.seq}>
                  <td>{ msg.raw.timestamp }</td>
                  <td>
                    <span className="text-muted">0x</span>
                    { msg.raw.id.device_type.toString(16).padStart(2, "0") }
                    { idAnnotation(msg.raw.id.device_type, DEVICE_TYPES) }
                  </td>
                  <td>
                    <span className="text-muted">0x</span>
                    { msg.raw.id.manufacturer.toString(16).padStart(2, "0") }
                    { idAnnotation(msg.raw.id.manufacturer, MANUFACTURERS) }
                  </td>
                  <td>
                    <span className="text-muted">0x</span>
                    { msg.raw.id.api_class.toString(16).padStart(2, "0") }
                  </td>
                  <td>
                    <span className="text-muted">0x</span>
                    { msg.raw.id.api_index.toString(16).padStart(2, "0") }
                  </td>
                  <td>{ msg.raw.id.device_id }</td>
                  <td>{ msg.raw.data.map(x => x.toString(16).padStart(2, "0")).join(" ") }</td>
                  <td>{ msg.grpl_defrag && <Button size="sm" className="p-0" variant="link" onClick={() => viewDecodeModal(msg.grpl_defrag)}>
                    <FontAwesomeIcon icon={ faMagnifyingGlass} />
                  </Button> }</td>
                </tr>
              ])
            }
          </tbody>
        </Table>
      </Col>
    </Row>
  </div>
}