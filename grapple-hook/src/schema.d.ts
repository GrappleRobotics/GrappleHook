/* eslint-disable */
/**
 * This file was automatically generated by json-schema-to-typescript.
 * DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
 * and run json-schema-to-typescript to regenerate this file.
 */

export type FirmwareUpgradeDeviceRequest =
  | {
      data: {
        data: number[];
      };
      method: "do_field_upgrade";
    }
  | {
      data: {};
      method: "progress";
    };
export type FirmwareUpgradeDeviceResponse =
  | {
      data: null;
      method: "do_field_upgrade";
    }
  | {
      data: number | null;
      method: "progress";
    };
export type LaserCanRequest =
  | {
      data: {};
      method: "start_field_upgrade";
    }
  | {
      data: {
        mode: LaserCanRangingMode;
      };
      method: "set_range";
    }
  | {
      data: {
        roi: LaserCanRoi;
      };
      method: "set_roi";
    }
  | {
      data: {
        budget: LaserCanTimingBudget;
      };
      method: "set_timing_budget";
    }
  | {
      data: {
        msg: GrappleDeviceRequest;
      };
      method: "grapple";
    }
  | {
      data: {};
      method: "status";
    };
export type LaserCanRangingMode = "Short" | "Long";
export type LaserCanTimingBudget = "TB20ms" | "TB33ms" | "TB50ms" | "TB100ms";
export type GrappleDeviceRequest =
  | {
      data: {};
      method: "blink";
    }
  | {
      data: {
        id: number;
      };
      method: "set_id";
    }
  | {
      data: {
        name: string;
      };
      method: "set_name";
    }
  | {
      data: {};
      method: "commit_to_eeprom";
    };
export type LaserCanResponse =
  | {
      data: null;
      method: "start_field_upgrade";
    }
  | {
      data: null;
      method: "set_range";
    }
  | {
      data: null;
      method: "set_roi";
    }
  | {
      data: null;
      method: "set_timing_budget";
    }
  | {
      data: GrappleDeviceResponse;
      method: "grapple";
    }
  | {
      data: LaserCanStatus;
      method: "status";
    };
export type GrappleDeviceResponse =
  | {
      data: null;
      method: "blink";
    }
  | {
      data: null;
      method: "set_id";
    }
  | {
      data: null;
      method: "set_name";
    }
  | {
      data: null;
      method: "commit_to_eeprom";
    };
export type OldVersionDeviceRequest =
  | {
      data: {};
      method: "start_field_upgrade";
    }
  | {
      data: {};
      method: "get_error";
    }
  | {
      data: {};
      method: "get_firmware_url";
    }
  | {
      data: {
        msg: GrappleDeviceRequest;
      };
      method: "grapple";
    };
export type OldVersionDeviceResponse =
  | {
      data: null;
      method: "start_field_upgrade";
    }
  | {
      data: string;
      method: "get_error";
    }
  | {
      data: string | null;
      method: "get_firmware_url";
    }
  | {
      data: GrappleDeviceResponse;
      method: "grapple";
    };
export type ProviderManagerRequest =
  | {
      data: {
        address: string;
      };
      method: "delete";
    }
  | {
      data: {
        address: string;
        msg: WrappedDeviceProviderRequest;
      };
      method: "provider";
    }
  | {
      data: {};
      method: "providers";
    };
export type WrappedDeviceProviderRequest =
  | {
      data: {};
      method: "connect";
    }
  | {
      data: {};
      method: "disconnect";
    }
  | {
      data: {};
      method: "info";
    }
  | {
      data: {
        req: DeviceManagerRequest;
      };
      method: "device_manager_call";
    };
export type DeviceManagerRequest =
  | {
      data: {
        data: unknown;
        device_id: DeviceId;
        domain: string;
      };
      method: "call";
    }
  | {
      data: {};
      method: "devices";
    };
export type DeviceId =
  | {
      Dfu: number;
    }
  | {
      Serial: number;
    };
export type ProviderManagerResponse =
  | {
      data: null;
      method: "delete";
    }
  | {
      data: WrappedDeviceProviderResponse;
      method: "provider";
    }
  | {
      data: {
        [k: string]: ProviderInfo;
      };
      method: "providers";
    };
export type WrappedDeviceProviderResponse =
  | {
      data: null;
      method: "connect";
    }
  | {
      data: null;
      method: "disconnect";
    }
  | {
      data: ProviderInfo;
      method: "info";
    }
  | {
      data: DeviceManagerResponse;
      method: "device_manager_call";
    };
export type DeviceManagerResponse =
  | {
      data: unknown;
      method: "call";
    }
  | {
      data: {
        [k: string]: [DeviceId, DeviceInfo, string][];
      };
      method: "devices";
    };
export type DeviceType =
  | ("RoboRIO" | "Unknown")
  | {
      Grapple: GrappleModelId;
    };
export type GrappleModelId = "LaserCan" | "SpiderLan";

export interface MegaSchema {
  firmware_req: FirmwareUpgradeDeviceRequest;
  firmware_rsp: FirmwareUpgradeDeviceResponse;
  lasercan_req: LaserCanRequest;
  lasercan_rsp: LaserCanResponse;
  old_version_req: OldVersionDeviceRequest;
  old_version_rsp: OldVersionDeviceResponse;
  provider_manager_req: ProviderManagerRequest;
  provider_manager_rsp: ProviderManagerResponse;
}
export interface LaserCanRoi {
  h: number;
  w: number;
  x: number;
  y: number;
}
export interface LaserCanStatus {
  last_update?: LaserCanMeasurement | null;
}
export interface LaserCanMeasurement {
  ambient: number;
  budget: LaserCanTimingBudget;
  distance_mm: number;
  mode: LaserCanRangingMode;
  roi: LaserCanRoi;
  status: number;
}
export interface ProviderInfo {
  address: string;
  connected: boolean;
  description: string;
}
export interface DeviceInfo {
  device_id?: number | null;
  device_type: DeviceType;
  firmware_version?: string | null;
  is_dfu: boolean;
  is_dfu_in_progress: boolean;
  name?: string | null;
  serial?: number | null;
}
