declare module "pcb-stackup" {
  interface StackupLayer {
    filename?: string;
    gerber: string;
  }
  interface StackupSide {
    svg: string;
  }
  interface Stackup {
    top: StackupSide;
    bottom: StackupSide;
  }
  export default function pcbStackup(
    layers: StackupLayer[],
    options?: Record<string, unknown>,
  ): Promise<Stackup>;
}
