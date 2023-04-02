import { faFlask } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import React from "react";
import { Alert } from "react-bootstrap";

export default class Experimental extends React.PureComponent {
  render() {
    return <Alert variant="purple">
      <FontAwesomeIcon icon={faFlask} style={{fontSize: '1.5em'}} /> &nbsp;
      <span style={{fontSize: '1.5em'}}>You've found an <strong>Experimental Feature</strong></span>
      <br />
      <span> Be careful with this. Although we try to test as much as we can, we make no guarantees about the performance of this feature. Use with caution! </span>
      <br />
      <span style={{fontSize: '0.9em'}}><i>Also, keep us in the loop! We want to know how it performs!</i></span>
    </Alert>
  }
}