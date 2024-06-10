import React, { useEffect, useState } from "react"
import { Button, Col, InputGroup, Nav, Row, Tab } from "react-bootstrap"
import ProviderComponent from "./Provider"
import { renderDeviceType, DeviceComponent } from "../devices/Device"
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faPlus } from "@fortawesome/free-solid-svg-icons"
import { confirmModal } from "../Confirm"
import BufferedFormControl from "../BufferedFormControl"
import { DeviceId, DeviceInfo, DeviceManagerRequest, DeviceManagerResponse, ProviderInfo, ProviderManagerRequest, ProviderManagerResponse, WrappedDeviceProviderRequest, WrappedDeviceProviderResponse } from "../schema"
import { useToasts } from "../toasts"
import { rpc } from "../rpc"
import update from "immutability-helper";

type ProviderManagerProps = {
  invoke: (msg: ProviderManagerRequest) => Promise<ProviderManagerResponse>,
}

export default function ProviderManagerComponent(props: ProviderManagerProps) {
  const { invoke } = props;
  const { addError } = useToasts();

  const [ providers, setProviders ] = useState<{ [key: string]: ProviderInfo }>({});
  const [ devices, setDevices ] = useState<{ [key: string]: { [domain: string]: [DeviceId, DeviceInfo, string][] } }>({});

  const provider_rpc = (address: string) => {
    return async (msg: WrappedDeviceProviderRequest) => {
      return await rpc<ProviderManagerRequest, ProviderManagerResponse, "provider">(invoke, "provider", {
        address: address, msg: msg
      })
    }
  }

  const device_manager_rpc = (provider_address: string) => {
    return async (msg: DeviceManagerRequest) => {
      return await rpc<WrappedDeviceProviderRequest, WrappedDeviceProviderResponse, "device_manager_call">(provider_rpc(provider_address), "device_manager_call", {
        req: msg
      })
    }
  }

  const device_rpc = (provider_address: string, domain: string, device_id: DeviceId) => {
    return async (msg: any) => {
      return await rpc<DeviceManagerRequest, DeviceManagerResponse, "call">(device_manager_rpc(provider_address), "call", {
        device_id: device_id, domain: domain, data: msg
      })
    }
  }

  useEffect(() => {
    const interval = setInterval(() => {
      rpc<ProviderManagerRequest, ProviderManagerResponse, "providers">(invoke, "providers", {})
        .then((providers) => {
          setProviders(providers);
          let futs = Promise.all(Object.keys(providers).map(provider => rpc<DeviceManagerRequest, DeviceManagerResponse, "devices">(device_manager_rpc(providers[provider].address), "devices", {}).catch(addError)));
          futs.then(vals => {
            let devs = update(devices, {});
            vals.forEach((v, i) => {
              if (v) {
                devs = update(devs, { [Object.keys(providers)[i]]: { $set: v } });
              }
            });
            setDevices(devs);
          });
        })
        .catch(addError)
    }, 500);
    return () => clearInterval(interval);
  }, [])

  return <React.Fragment>
    <Tab.Container>
      <Row>
        <Col md={4}>
          <Nav variant="pills" className="flex-column">
            {
              Object.keys(providers).map(key => {
                let p = providers[key];
                return [
                  <Nav.Item className="device-list-provider">
                    <Nav.Link eventKey={"provider-" + key}>
                      <span style={{fontSize: '1.5em'}}>{ p.description }</span> <br />
                      <span className="text-muted">{ p.address }</span> &nbsp;
                      <span className={ p.connected ? "text-success" : "text-danger" }>{ p.connected ? "CONNECTED" : "DISCONNECTED" }</span>
                    </Nav.Link>
                  </Nav.Item>,
                  ...Object.keys(devices[key] || {}).flatMap(domain => [
                    devices[key][domain].map(([device_id, device_info, device_class]) => (
                      <DevicePillComponent provider_key={key} domain={domain} device_id={device_id} device_info={device_info} device_class={device_class} />
                    ))
                  ])
                ]
              })
            }
          </Nav>
        </Col>
        <Col md={8}>
          <Tab.Content>
            {
              Object.keys(providers).map(key => {
                let p = providers[key];
                return [
                  <Tab.Pane eventKey={"provider-" + key}>
                    <ProviderComponent info={p} invoke={provider_rpc(p.address)} />
                  </Tab.Pane>,
                  ...Object.keys(devices[key] || {}).flatMap(domain => [
                    devices[key][domain].map(([device_id, device_info, device_class]) => (
                      <Tab.Pane eventKey={`device-${key}-${domain}-${JSON.stringify(device_id)}`}>
                        <DeviceComponent id={device_id} info={device_info} device_class={device_class} invoke={device_rpc(p.address, domain, device_id)} />
                      </Tab.Pane>
                    ))
                  ])
                ]
              })
            }
          </Tab.Content>
        </Col>
      </Row>
    </Tab.Container>
  </React.Fragment>
}

export function DevicePillComponent(props: { provider_key: string, domain: string, device_id: DeviceId, device_info: DeviceInfo, device_class: string }) {
  const { provider_key, domain, device_id, device_info } = props;
  return <Nav.Item className="device-list-device">
     <Nav.Link eventKey={`device-${provider_key}-${domain}-${JSON.stringify(device_id)}`}>
       {
         device_info.is_dfu ? <React.Fragment>
           { renderDeviceType(device_info.device_type) } &nbsp;
           <span className="text-orange">F/W UPDATE</span>
           <br />
           <span className="tip">
             { domain } &nbsp;
             { device_info.serial != undefined && `Serial: 0x${device_info.serial.toString(16)}` } &nbsp;
             { device_info.firmware_version != undefined && `BL: ${device_info.firmware_version}` }
           </span>
         </React.Fragment> : <React.Fragment>
           { device_info.device_id != undefined && `#${device_info.device_id}` } &nbsp;
           { renderDeviceType(device_info.device_type) } &nbsp;
           { device_info.name != undefined && `(${device_info.name})` }
           <br />
           <span className="tip">
             { domain } &nbsp;
             { device_info.serial != undefined && `Serial: 0x${device_info.serial.toString(16)}` } &nbsp;
             { device_info.firmware_version != undefined && `FW: ${device_info.firmware_version}` }
           </span>
         </React.Fragment>
       }    
     </Nav.Link>
   </Nav.Item>
}
