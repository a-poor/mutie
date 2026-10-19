export type Flag = true | false;
export interface State { enabled: true; disabled: false }
export type Choice<T extends boolean = true> = T extends true ? true : false;
