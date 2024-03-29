import React from "react";
import { OverlayTrigger, Tooltip, TooltipProps } from "react-bootstrap";
import { Combine } from "./util";

type SimpleTooltipProps = Combine<{
  id: string,
  tip: React.ReactNode | string,
  children: React.ReactNode,
  disabled?: boolean,
  tipClass?: string,
}, TooltipProps>;

export default class SimpleTooltip extends React.PureComponent<SimpleTooltipProps> {
  render() {
    let { id, tip, children, placement, disabled, tipClass, ...props } = this.props;
    return disabled ? <span> { children } </span> : <OverlayTrigger
      placement={placement || "top"}
      overlay={
        <Tooltip id={id} {...props}>
          { tip }
        </Tooltip>
      }
    >
      <span className={tipClass}> { children } </span>
    </OverlayTrigger>
  }
}