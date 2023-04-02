export const rpc = async <Req extends { method: string, data: any }, Rsp extends { method: string, data: any }, Method extends Req["method"]>
(fn: (req: Req) => Promise<Rsp>, method: Method, args: Extract<Req, { method: Method }>["data"]) : Promise<Extract<Rsp, { method: Method }>["data"]> => {
  return (await fn({ method: method, data: args } as any as Req) as any)["data"]
}