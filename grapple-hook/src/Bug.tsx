import { faBug } from "@fortawesome/free-solid-svg-icons"
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import React from "react"
import { Accordion, Alert } from "react-bootstrap"

type BugProperties = {
  specifics: React.ReactNode,
  dump?: any,
  showReport?: boolean,
  titleText?: React.ReactNode,
}

export default class Bug extends React.PureComponent<BugProperties> {
  render() {
    return <Alert variant="danger">
      <FontAwesomeIcon icon={faBug} size="2x" /> &nbsp;
      <span style={{ fontSize: '2em' }}>{ this.props.titleText || <React.Fragment><strong>Oops!</strong> You've found a bug!</React.Fragment> }</span>
      <br />
      { this.props.showReport === undefined || this.props.showReport === true ? <React.Fragment>
        <span>Have you updated GrappleHook? If so, you should report this! </span>
        <br />
      </React.Fragment> : <React.Fragment /> }
      <strong>{ this.props.specifics }</strong>
      {
        this.props.dump != undefined && <Accordion className="mt-2">
          <Accordion.Item eventKey="0">
            <Accordion.Header>Dump</Accordion.Header>
            <Accordion.Body>
              <pre>
                { JSON.stringify(this.props.dump, null, 2) }
              </pre>
            </Accordion.Body>
          </Accordion.Item>
        </Accordion>
      }
    </Alert>
  }
}