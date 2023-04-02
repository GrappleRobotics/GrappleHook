import { faInfoCircle, faTriangleExclamation } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import React from "react";

type HintProps = {
  type?: "info" | "warning" | "danger" | "highlight",
  children: React.ReactNode
};

export default class Hint extends React.PureComponent<HintProps> {
  render() {
    const type = this.props.type || "info";
    return <span className={ type == "info" ? "text-muted" : type == "highlight" ? "text-info" : type == "warning" ? "text-warning" : "text-danger" } style={{ fontSize: '0.9em' }}>
      <FontAwesomeIcon icon={(type == "info" || type == "highlight") ? faInfoCircle : faTriangleExclamation} />
      { this.props.children }
    </span>
  }
}