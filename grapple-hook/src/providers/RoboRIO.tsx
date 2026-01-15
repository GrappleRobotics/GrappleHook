import { useEffect, useState } from "react";
import { rpc } from "../rpc";
import { ProviderInfo, RoboRioDaemonRequest, RoboRioDaemonResponse, RoboRIOStatus } from "../schema"
import Bug from "../Bug";
import { Col, FormLabel, Row, Tab, Tabs } from "react-bootstrap";
import EnumToggleGroup from "../EnumToggleGroup";
import BufferedFormControl from "../BufferedFormControl";
import { CodeBlock } from "react-code-blocks";
import { useDebugCtx } from "../DebugContext";
import CanLog from "../debug/CanLog";

export type RoboRIOProps = {
  info: ProviderInfo,
  invoke: (msg: RoboRioDaemonRequest) => Promise<RoboRioDaemonResponse>
}

export default function RoboRIO(props: RoboRIOProps) {
  const { info, invoke } = props;

  const { mode: debugMode } = useDebugCtx();

  const [ status, setStatus ] = useState<RoboRIOStatus>();

  useEffect(() => {
    const interval = setInterval(() => {
      rpc<RoboRioDaemonRequest, RoboRioDaemonResponse, "status">(invoke, "status", {})
        .then(setStatus)
        .catch(e => {});  // Discard, it's usually a message to say that the device is disconnected and the UI fragment just hasn't been evicted yet.
    }, 50);
    return () => clearInterval(interval);
  }, []);

  const example_code_java = `  // Java
  import au.grapplerobotics.CanBridge;
  public class Robot extends <your robot template> {
    public Robot() {
      CanBridge.runTCP();
      // ...
    }
  }`

  const example_code_cpp = `  // C++
  #include "grpl/CanBridge.h"

  class Robot : public <your robot template> {
  public:
    Robot() {
      grpl::start_can_bridge();
      // ...
    }
  }`

  const example_code_python = `  # Python
  from libgrapplefrc import can_bridge_tcp

  class MyRobot(<your robot template>):
    def robotInit(self):
      can_bridge_tcp()
      # ...
  `

  const canLogEnabled = debugMode == "debug" && info.connected;

  return <div>
    {
      <Row style={{ display: canLogEnabled ? "" : "none" }}>
        <Col>
          <CanLog enabled={canLogEnabled} invoke={msg => rpc<RoboRioDaemonRequest, RoboRioDaemonResponse, "canlog_call">(invoke, "canlog_call", { req: msg })} />
        </Col>
      </Row>
    }

    <Row>
      <Col>
        <Bug
          titleText={<strong>Hold on!</strong>}
          showReport={false}
          specifics={
            <span>
              We're aware of an issue where the RoboRIO sometimes fails to accept the GrappleHook daemon binary.
              Please select <span className="text-primary"> USER CODE </span> below and follow the instructions.  <br />
              If trouble persists, see: <a className="text-primary" href="https://github.com/GrappleRobotics/GrappleHook/issues/2">this GitHub issue</a> and leave a comment.
            </span>
          }
        />
      </Col>
    </Row>
    <Row>
      <Col md="auto">
        <EnumToggleGroup name="connection-type" values={[true, false]} names={["Deploy Daemon", "User Code"]} value={status?.using_daemon} variantActive="success" variant="secondary" onChange={() => invoke({ method: "set_use_daemon", data: { use_daemon: !status?.using_daemon } })} />
      </Col>
      {/* <Col>
        <FormLabel>RoboRIO IP Address</FormLabel>
        <BufferedFormControl type="text" value={info?.address} onChange={(v) => invoke({ method: "set_address", data: { address: String(v) } })} />
      </Col> */}
    </Row>
    {
      !status?.using_daemon && <Row className="mt-3">
        <Col>
          You've selected <strong><span className="text-success"> USER CODE </span></strong>
          <br />

          In your Robot Code, merge in the following changes, redeploy, and then try and connect again. <br />
          <strong>Make sure <span className="text-primary">libgrapplefrc</span> is up to date! (at least 2025.0.5)</strong> <br /> <br />

          <Tabs id="lang-examples" defaultActiveKey="java">
            <Tab eventKey="java" title="JAVA">
              <CodeBlock language="java" text={example_code_java} showLineNumbers={false} />
            </Tab>
            <Tab eventKey="c++" title="C++">
              <CodeBlock language="c++" text={example_code_cpp} showLineNumbers={false} />
            </Tab>
            <Tab eventKey="python" title="Python">
              <CodeBlock language="python" text={example_code_python} showLineNumbers={false} />
            </Tab>
          </Tabs>
        </Col>
      </Row>
    }
  </div>
}